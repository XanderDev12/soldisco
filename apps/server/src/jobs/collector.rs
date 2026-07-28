use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::{Stream, StreamExt as _, stream};
use soldisco_domain::{ChainCoordinate, Commitment, Network, SourceProgram};
use soldisco_persistence::{CollectionPosition, Database, NewCollectionGap, RecoveryCheckpoint};
use soldisco_solana_rpc::{
    ProgramLogNotification, ReadContext, RecoveryCheckpoint as RpcRecoveryCheckpoint,
    RecoveryPager, RecoveryProgress, RpcError, SignatureRecord, SolanaHttpClient,
    SolanaPubsubClient, SolanaReader, TransactionInstructionRecord, TransactionRecord,
};
use soldisco_source_pump::PumpProgram;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const MAX_RPC_RETRY_DELAY: Duration = Duration::from_secs(30);
const PUMP_SWAP_RETRY_OFFSET: Duration = Duration::from_millis(250);
const CHECKPOINT_KIND: &str = "PROGRAM_LOGS";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectorSubscription {
    pub source_program: SourceProgram,
    pub program_id: String,
}

/// One Anchor `Program data:` record attributed to an exact top-level
/// transaction instruction. Event indexes are scoped to that instruction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScopedProgramData {
    pub coordinate: ChainCoordinate,
    pub log: String,
}

#[derive(Clone, Debug)]
pub struct CollectorRuntimeConfig {
    pub network: Network,
    pub commitment: Commitment,
    pub reconnect_delay: Duration,
    pub request_timeout: Duration,
    pub rpc_max_in_flight: usize,
    pub live_fetch_max_attempts: usize,
    pub subscription_idle_timeout: Duration,
    pub recovery_page_size: usize,
    pub recovery_max_records: usize,
}

#[derive(Clone)]
pub struct ProgramSourceContext {
    pub database: Database,
    pub http: Arc<SolanaHttpClient>,
    pub pubsub: SolanaPubsubClient,
    pub batches: mpsc::Sender<ProgramLogBatch>,
    pub connections: mpsc::UnboundedSender<SourceConnectionUpdate>,
    pub config: CollectorRuntimeConfig,
}

#[derive(Clone, Debug)]
pub struct ProgramLogBatch {
    pub program: PumpProgram,
    pub slot: u64,
    pub transaction_index: Option<u64>,
    pub signature: String,
    /// Wall-clock time at which this source record first entered the collector,
    /// before any authoritative HTTP fetch or retry delay.
    pub received_time_unix_ms: i64,
    pub instructions: Vec<TransactionInstructionRecord>,
    pub log_messages: Vec<String>,
    pub transaction_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceConnectionState {
    Connecting,
    Recovering,
    Ready,
    ReadyWithGap,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceConnectionUpdate {
    pub source_program: SourceProgram,
    pub state: SourceConnectionState,
}

#[derive(Debug, Error)]
pub enum CollectorError {
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error(transparent)]
    Persistence(#[from] soldisco_persistence::PersistenceError),
    #[error("collector processing queue is closed")]
    ProcessingQueueClosed,
    #[error(
        "{source_program:?} transaction {signature} remained unavailable after {attempts} attempts: {source}"
    )]
    LiveFetchExhausted {
        source_program: SourceProgram,
        signature: String,
        attempts: usize,
        #[source]
        source: RpcError,
    },
}

#[derive(Clone, Debug)]
struct ReceivedProgramLogNotification {
    notification: ProgramLogNotification,
    received_time_unix_ms: i64,
}

#[derive(Debug)]
enum LiveSourceExit {
    Subscription(RpcError),
    Fetch(CollectorError),
    Ended,
}

#[derive(Clone)]
struct LiveFetchHealth {
    state: Arc<Mutex<LiveFetchHealthState>>,
    connections: mpsc::UnboundedSender<SourceConnectionUpdate>,
    source_program: SourceProgram,
    ready_state: SourceConnectionState,
}

#[derive(Default)]
struct LiveFetchHealthState {
    active_retries: usize,
    exhausted: bool,
}

struct LiveProcessingContext<R> {
    http: Arc<R>,
    batches: mpsc::Sender<ProgramLogBatch>,
    connections: mpsc::UnboundedSender<SourceConnectionUpdate>,
    cancellation: CancellationToken,
    config: CollectorRuntimeConfig,
    ready_state: SourceConnectionState,
}

impl LiveFetchHealth {
    fn new(
        connections: mpsc::UnboundedSender<SourceConnectionUpdate>,
        source_program: SourceProgram,
        ready_state: SourceConnectionState,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(LiveFetchHealthState::default())),
            connections,
            source_program,
            ready_state,
        }
    }

    fn begin_retry(&self) -> LiveFetchRetryGuard {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.active_retries == 0 {
            send_connection(
                &self.connections,
                self.source_program,
                SourceConnectionState::Recovering,
            );
        }
        state.active_retries = state.active_retries.saturating_add(1);
        drop(state);
        LiveFetchRetryGuard {
            health: self.clone(),
            recovered: false,
        }
    }
}

struct LiveFetchRetryGuard {
    health: LiveFetchHealth,
    recovered: bool,
}

impl LiveFetchRetryGuard {
    fn recovered(mut self) {
        self.recovered = true;
    }

    fn exhausted(&self) {
        self.health
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .exhausted = true;
    }
}

impl Drop for LiveFetchRetryGuard {
    fn drop(&mut self) {
        let mut state = self
            .health
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        debug_assert!(
            state.active_retries > 0,
            "live retry guard count must not underflow"
        );
        state.active_retries = state.active_retries.saturating_sub(1);
        if state.active_retries == 0 && self.recovered && !state.exhausted {
            send_connection(
                &self.health.connections,
                self.health.source_program,
                self.health.ready_state,
            );
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LogScopeError {
    #[error("transaction signature cannot be empty")]
    EmptySignature,
    #[error("program invocation depth {depth} is inconsistent with the log stack")]
    InvalidInvocationDepth { depth: usize },
    #[error("top-level transaction contains more than {0} instructions")]
    TooManyInstructions(u16),
    #[error("one transaction instruction contains more than {0} program-data events")]
    TooManyEvents(u16),
}

/// Walk Solana execution logs and keep data emitted only while `program_id` is
/// the active invocation. A depth-one `invoke` is the exact top-level
/// transaction instruction boundary; nested CPIs inherit that index.
pub fn scope_program_data_logs(
    program_id: &str,
    slot: u64,
    transaction_index: Option<u64>,
    signature: &str,
    logs: &[String],
) -> Result<Vec<ScopedProgramData>, LogScopeError> {
    if signature.trim().is_empty() {
        return Err(LogScopeError::EmptySignature);
    }

    let mut stack: Vec<(String, u16)> = Vec::new();
    let mut next_instruction_index = 0_u16;
    let mut event_indexes = BTreeMap::<u16, u16>::new();
    let mut scoped = Vec::new();

    for log in logs {
        if let Some((invoked_program, depth)) = parse_invoke(log) {
            if depth == 0 || depth > stack.len().saturating_add(1) {
                return Err(LogScopeError::InvalidInvocationDepth { depth });
            }

            if depth == 1 {
                stack.clear();
                let instruction_index = next_instruction_index;
                next_instruction_index = next_instruction_index
                    .checked_add(1)
                    .ok_or(LogScopeError::TooManyInstructions(u16::MAX))?;
                stack.push((invoked_program.to_owned(), instruction_index));
            } else {
                stack.truncate(depth - 1);
                let instruction_index = stack
                    .last()
                    .map(|(_, instruction_index)| *instruction_index)
                    .ok_or(LogScopeError::InvalidInvocationDepth { depth })?;
                stack.push((invoked_program.to_owned(), instruction_index));
            }
            continue;
        }

        if let Some(exited_program) = parse_exit(log) {
            if stack
                .last()
                .is_some_and(|(active_program, _)| active_program == exited_program)
            {
                stack.pop();
            }
            continue;
        }

        if !log.starts_with("Program data: ") {
            continue;
        }
        let Some((active_program, instruction_index)) = stack.last() else {
            continue;
        };
        if active_program != program_id {
            continue;
        }

        let event_index = event_indexes.entry(*instruction_index).or_default();
        let coordinate = ChainCoordinate {
            slot,
            transaction_index,
            signature: signature.to_owned(),
            instruction_index: *instruction_index,
            event_index: *event_index,
        };
        *event_index = event_index
            .checked_add(1)
            .ok_or(LogScopeError::TooManyEvents(u16::MAX))?;
        scoped.push(ScopedProgramData {
            coordinate,
            log: log.clone(),
        });
    }

    Ok(scoped)
}

pub async fn run_program_source(
    program: PumpProgram,
    context: ProgramSourceContext,
    cancellation: CancellationToken,
) -> Result<(), CollectorError> {
    let source_program = program.source_program();
    let mut retry_delay = context.config.reconnect_delay;
    loop {
        if cancellation.is_cancelled() {
            return Ok(());
        }
        send_connection(
            &context.connections,
            source_program,
            SourceConnectionState::Connecting,
        );

        let subscription = tokio::time::timeout(
            context.config.request_timeout,
            context
                .pubsub
                .subscribe_program_logs(program.program_id(), context.config.commitment),
        )
        .await;
        let subscription = match subscription {
            Ok(Ok(subscription)) => subscription,
            Ok(Err(error)) => {
                tracing::warn!(
                    source = ?source_program,
                    %error,
                    "Solana program-log subscription failed; retrying"
                );
                send_connection(
                    &context.connections,
                    source_program,
                    SourceConnectionState::Disconnected,
                );
                wait_to_reconnect(&cancellation, retry_delay).await?;
                retry_delay = next_retry_delay(retry_delay, context.config.reconnect_delay);
                continue;
            }
            Err(_) => {
                tracing::warn!(
                    source = ?source_program,
                    "Solana program-log subscription timed out; retrying"
                );
                send_connection(
                    &context.connections,
                    source_program,
                    SourceConnectionState::Disconnected,
                );
                wait_to_reconnect(&cancellation, retry_delay).await?;
                retry_delay = next_retry_delay(retry_delay, context.config.reconnect_delay);
                continue;
            }
        };

        send_connection(
            &context.connections,
            source_program,
            SourceConnectionState::Recovering,
        );
        let recovery_had_gap = match recover_program(
            program,
            &context.database,
            &context.http,
            &context.batches,
            &cancellation,
            &context.config,
        )
        .await
        {
            Ok(()) => false,
            Err(error) if is_unrecoverable_gap(&error) => {
                let gap_id =
                    record_unrecoverable_gap(program, &context.database, &context.config, &error)
                        .await?;
                tracing::error!(
                    source = ?source_program,
                    gap_id,
                    %error,
                    "bounded recovery could not reach the checkpoint; live collection will resume with an explicit history gap"
                );
                true
            }
            Err(error) => {
                tracing::warn!(
                    source = ?source_program,
                    %error,
                    "Solana recovery failed; reconnecting before live release"
                );
                send_connection(
                    &context.connections,
                    source_program,
                    SourceConnectionState::Disconnected,
                );
                wait_to_reconnect(&cancellation, retry_delay).await?;
                retry_delay = next_retry_delay(retry_delay, context.config.reconnect_delay);
                continue;
            }
        };

        retry_delay = context.config.reconnect_delay;
        let has_active_gap = recovery_had_gap
            || context
                .database
                .count_active_collection_gaps(context.config.network, source_program)
                .await?
                > 0;
        let ready_state = if has_active_gap {
            SourceConnectionState::ReadyWithGap
        } else {
            SourceConnectionState::Ready
        };
        send_connection(&context.connections, source_program, ready_state);
        let notifications =
            live_notification_stream(subscription, context.config.subscription_idle_timeout);
        let exit = process_live_notifications(
            program,
            notifications,
            LiveProcessingContext {
                http: context.http.clone(),
                batches: context.batches.clone(),
                connections: context.connections.clone(),
                cancellation: cancellation.clone(),
                config: context.config.clone(),
                ready_state,
            },
        )
        .await?;
        if cancellation.is_cancelled() {
            return Ok(());
        }
        match exit {
            LiveSourceExit::Subscription(error) => {
                tracing::warn!(
                    source = ?source_program,
                    %error,
                    "Solana program-log subscription disconnected"
                );
            }
            LiveSourceExit::Fetch(error) => {
                tracing::warn!(
                    source = ?source_program,
                    %error,
                    "authoritative live transaction fetch was exhausted; reconnecting for recovery"
                );
            }
            LiveSourceExit::Ended => {
                tracing::warn!(
                    source = ?source_program,
                    "Solana program-log subscription ended unexpectedly"
                );
            }
        }
        send_connection(
            &context.connections,
            source_program,
            SourceConnectionState::Disconnected,
        );

        wait_to_reconnect(&cancellation, retry_delay).await?;
        retry_delay = next_retry_delay(retry_delay, context.config.reconnect_delay);
    }
}

async fn recover_program(
    program: PumpProgram,
    database: &Database,
    http: &SolanaHttpClient,
    batches: &mpsc::Sender<ProgramLogBatch>,
    cancellation: &CancellationToken,
    config: &CollectorRuntimeConfig,
) -> Result<(), CollectorError> {
    let Some(checkpoint) = database
        .load_recovery_checkpoint(config.network, program.source_program(), CHECKPOINT_KIND)
        .await?
    else {
        // A first run begins at the already-open live subscription. The first
        // durably processed notification establishes the checkpoint without
        // pretending the complete historical firehose was ingested.
        return Ok(());
    };

    let mut pager = RecoveryPager::after_checkpoint(
        RpcRecoveryCheckpoint {
            program_id: program.program_id().to_owned(),
            last_slot: checkpoint.last_slot,
            last_transaction_index: checkpoint.last_transaction_index,
            last_signature: checkpoint.last_signature,
        },
        config.recovery_page_size,
        config.recovery_max_records,
        ReadContext {
            commitment: config.commitment,
            minimum_slot: None,
        },
    )?;

    let batch = loop {
        let page = tokio::time::timeout(
            config.request_timeout,
            http.signature_page(pager.next_request()?),
        )
        .await
        .map_err(|_| RpcError::Timeout)??;
        match pager.accept_page(page)? {
            RecoveryProgress::More => {}
            RecoveryProgress::Complete(batch) => break batch,
        }
    };

    let recovered = stream::iter(batch.records_oldest_first)
        .map(|signature| fetch_recovery_record(http, program, signature, config))
        .buffered(config.rpc_max_in_flight);
    futures_util::pin_mut!(recovered);
    loop {
        let next = tokio::select! {
            () = cancellation.cancelled() => return Ok(()),
            next = recovered.next() => next,
        };
        let Some(batch) = next else {
            break;
        };
        send_raw_batch(batches, batch?, cancellation).await?;
    }

    Ok(())
}

async fn fetch_recovery_record<R: SolanaReader + ?Sized>(
    http: &R,
    program: PumpProgram,
    signature: SignatureRecord,
    config: &CollectorRuntimeConfig,
) -> Result<ProgramLogBatch, CollectorError> {
    let received_time_unix_ms = unix_time_millis();
    if !signature.succeeded() {
        return Ok(ProgramLogBatch {
            program,
            slot: signature.slot,
            transaction_index: signature.transaction_index,
            signature: signature.signature,
            received_time_unix_ms,
            instructions: Vec::new(),
            log_messages: Vec::new(),
            transaction_error: signature.transaction_error,
        });
    }

    let mut transaction = tokio::time::timeout(
        config.request_timeout,
        http.transaction(
            &signature.signature,
            ReadContext {
                commitment: config.commitment,
                minimum_slot: Some(signature.slot),
            },
        ),
    )
    .await
    .map_err(|_| RpcError::Timeout)??
    .ok_or_else(|| {
        RpcError::Unavailable(format!(
            "transaction {} disappeared during recovery",
            signature.signature
        ))
    })?;
    if transaction.slot != signature.slot {
        return Err(RpcError::InvalidResponse(
            "recovery transaction slot did not match its signature record".to_owned(),
        )
        .into());
    }
    match (signature.transaction_index, transaction.transaction_index) {
        (Some(expected), Some(actual)) if expected != actual => {
            return Err(RpcError::InvalidResponse(
                "recovery transaction index did not match its signature record".to_owned(),
            )
            .into());
        }
        (Some(expected), None) => transaction.transaction_index = Some(expected),
        _ => {}
    }
    Ok(transaction_batch(
        program,
        transaction,
        received_time_unix_ms,
    ))
}

fn live_notification_stream(
    subscription: soldisco_solana_rpc::ProgramLogSubscription,
    idle_timeout: Duration,
) -> impl Stream<Item = Result<ReceivedProgramLogNotification, RpcError>> {
    stream::unfold(Some(subscription), move |state| async move {
        let mut subscription = state?;
        let received =
            match tokio::time::timeout(idle_timeout, subscription.next_notification()).await {
                Ok(result) => result.map(|notification| ReceivedProgramLogNotification {
                    notification,
                    received_time_unix_ms: unix_time_millis(),
                }),
                Err(_) => Err(RpcError::SubscriptionClosed(format!(
                    "no program-log notification arrived within {} ms",
                    idle_timeout.as_millis()
                ))),
            };
        let next_state = received.is_ok().then_some(subscription);
        Some((received, next_state))
    })
}

async fn process_live_notifications<R, S>(
    program: PumpProgram,
    notifications: S,
    context: LiveProcessingContext<R>,
) -> Result<LiveSourceExit, CollectorError>
where
    R: SolanaReader + 'static,
    S: Stream<Item = Result<ReceivedProgramLogNotification, RpcError>>,
{
    let health = LiveFetchHealth::new(
        context.connections.clone(),
        program.source_program(),
        context.ready_state,
    );
    let rpc_max_in_flight = context.config.rpc_max_in_flight;
    let fetches = notifications
        .map(|received| {
            let http = context.http.clone();
            let health = health.clone();
            let config = context.config.clone();
            async move {
                match received {
                    Ok(received) => {
                        fetch_live_batch(http.as_ref(), program, received, &config, Some(&health))
                            .await
                            .map_err(LiveSourceExit::Fetch)
                    }
                    Err(error) => Err(LiveSourceExit::Subscription(error)),
                }
            }
        })
        .buffered(rpc_max_in_flight);
    futures_util::pin_mut!(fetches);

    loop {
        let fetched = tokio::select! {
            () = context.cancellation.cancelled() => return Ok(LiveSourceExit::Ended),
            fetched = fetches.next() => fetched,
        };
        match fetched {
            Some(Ok(batch)) => {
                send_raw_batch(&context.batches, batch, &context.cancellation).await?
            }
            Some(Err(exit)) => return Ok(exit),
            None => return Ok(LiveSourceExit::Ended),
        }
    }
}

async fn fetch_live_batch<R: SolanaReader + ?Sized>(
    http: &R,
    program: PumpProgram,
    received: ReceivedProgramLogNotification,
    config: &CollectorRuntimeConfig,
    health: Option<&LiveFetchHealth>,
) -> Result<ProgramLogBatch, CollectorError> {
    let notification = received.notification;

    let mut retry_delay = config.reconnect_delay;
    let mut last_error = None;
    let mut retry_guard: Option<LiveFetchRetryGuard> = None;
    for attempt in 1..=config.live_fetch_max_attempts {
        let result = tokio::time::timeout(
            config.request_timeout,
            http.transaction(
                &notification.signature,
                ReadContext {
                    commitment: config.commitment,
                    minimum_slot: Some(notification.slot),
                },
            ),
        )
        .await;
        match result {
            Ok(Ok(Some(transaction))) => {
                let validation_error = if transaction.slot != notification.slot {
                    Some(RpcError::InvalidResponse(
                        "live transaction slot did not match its WebSocket notification".to_owned(),
                    ))
                } else if transaction.succeeded() != notification.succeeded() {
                    Some(RpcError::InvalidResponse(
                        "live transaction status did not match its WebSocket notification"
                            .to_owned(),
                    ))
                } else {
                    None
                };
                if let Some(error) = validation_error {
                    tracing::warn!(
                        source = ?program.source_program(),
                        signature = %notification.signature,
                        attempt,
                        %error,
                        "authoritative transaction did not match its WebSocket notification"
                    );
                    last_error = Some(error);
                } else {
                    if let Some(guard) = retry_guard.take() {
                        guard.recovered();
                    }
                    return Ok(transaction_batch(
                        program,
                        transaction,
                        received.received_time_unix_ms,
                    ));
                }
            }
            Ok(Ok(None)) => {
                last_error = Some(RpcError::Unavailable(
                    "notified transaction was not yet available".to_owned(),
                ));
                tracing::debug!(
                    source = ?program.source_program(),
                    signature = %notification.signature,
                    attempt,
                    "notified transaction is not yet available from HTTP RPC"
                );
            }
            Ok(Err(error)) => {
                tracing::warn!(
                    source = ?program.source_program(),
                    signature = %notification.signature,
                    attempt,
                    %error,
                    "authoritative transaction fetch failed"
                );
                last_error = Some(error);
            }
            Err(_) => {
                tracing::warn!(
                    source = ?program.source_program(),
                    signature = %notification.signature,
                    attempt,
                    "authoritative transaction fetch timed out"
                );
                last_error = Some(RpcError::Timeout);
            }
        }

        if attempt < config.live_fetch_max_attempts {
            if retry_guard.is_none() {
                retry_guard = health.map(LiveFetchHealth::begin_retry);
            }
            let staggered_delay = if program == PumpProgram::PumpSwap {
                retry_delay.saturating_add(PUMP_SWAP_RETRY_OFFSET)
            } else {
                retry_delay
            };
            tokio::time::sleep(staggered_delay).await;
            retry_delay = next_retry_delay(retry_delay, config.reconnect_delay);
        }
    }

    if let Some(guard) = retry_guard.as_ref() {
        guard.exhausted();
    }
    Err(CollectorError::LiveFetchExhausted {
        source_program: program.source_program(),
        signature: notification.signature,
        attempts: config.live_fetch_max_attempts,
        source: last_error.expect("one or more live fetch attempts always record an error"),
    })
}

fn transaction_batch(
    program: PumpProgram,
    transaction: TransactionRecord,
    received_time_unix_ms: i64,
) -> ProgramLogBatch {
    ProgramLogBatch {
        program,
        slot: transaction.slot,
        transaction_index: transaction.transaction_index,
        signature: transaction.signature,
        received_time_unix_ms,
        instructions: transaction.instructions,
        log_messages: transaction.log_messages,
        transaction_error: transaction.transaction_error,
    }
}

async fn send_raw_batch(
    batches: &mpsc::Sender<ProgramLogBatch>,
    batch: ProgramLogBatch,
    cancellation: &CancellationToken,
) -> Result<(), CollectorError> {
    tokio::select! {
        () = cancellation.cancelled() => Ok(()),
        result = batches.send(batch) => result.map_err(|_| CollectorError::ProcessingQueueClosed),
    }
}

fn send_connection(
    connections: &mpsc::UnboundedSender<SourceConnectionUpdate>,
    source_program: SourceProgram,
    state: SourceConnectionState,
) {
    let _ = connections.send(SourceConnectionUpdate {
        source_program,
        state,
    });
}

async fn wait_to_reconnect(
    cancellation: &CancellationToken,
    delay: Duration,
) -> Result<(), CollectorError> {
    tokio::select! {
        () = cancellation.cancelled() => Ok(()),
        () = tokio::time::sleep(delay) => Ok(()),
    }
}

async fn record_unrecoverable_gap(
    program: PumpProgram,
    database: &Database,
    config: &CollectorRuntimeConfig,
    error: &CollectorError,
) -> Result<i64, CollectorError> {
    let source_program = program.source_program();
    let checkpoint = database
        .load_recovery_checkpoint(config.network, source_program, CHECKPOINT_KIND)
        .await?
        .ok_or_else(|| {
            RpcError::RecoveryProtocol(
                "recovery reported a history gap without a durable checkpoint".to_owned(),
            )
        })?;
    let reason_code = match error {
        CollectorError::Rpc(RpcError::RecoveryCheckpointNotFound { .. }) => {
            "RECOVERY_CHECKPOINT_NOT_FOUND"
        }
        CollectorError::Rpc(RpcError::RecoveryLimitExceeded { .. }) => "RECOVERY_LIMIT_EXCEEDED",
        _ => {
            return Err(RpcError::RecoveryProtocol(
                "attempted to record a non-gap collector error as a history gap".to_owned(),
            )
            .into());
        }
    };

    database
        .record_collection_gap(&NewCollectionGap {
            network: config.network,
            source_program,
            reason_code: reason_code.to_owned(),
            prior_checkpoint: checkpoint_position(checkpoint),
            detected_at_unix_ms: unix_time_millis(),
            details: format!("bounded recovery could not release a complete batch: {error}"),
        })
        .await
        .map_err(CollectorError::from)
}

fn checkpoint_position(checkpoint: RecoveryCheckpoint) -> CollectionPosition {
    CollectionPosition {
        slot: checkpoint.last_slot,
        transaction_index: checkpoint.last_transaction_index,
        signature: checkpoint.last_signature,
    }
}

fn is_unrecoverable_gap(error: &CollectorError) -> bool {
    matches!(
        error,
        CollectorError::Rpc(
            RpcError::RecoveryCheckpointNotFound { .. } | RpcError::RecoveryLimitExceeded { .. }
        )
    )
}

fn next_retry_delay(current: Duration, configured_minimum: Duration) -> Duration {
    current
        .saturating_mul(2)
        .min(MAX_RPC_RETRY_DELAY.max(configured_minimum))
}

fn unix_time_millis() -> i64 {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    i64::try_from(milliseconds).unwrap_or(i64::MAX)
}

fn parse_invoke(log: &str) -> Option<(&str, usize)> {
    let suffix = log.strip_prefix("Program ")?;
    let (program_id, depth) = suffix.split_once(" invoke [")?;
    let depth = depth.strip_suffix(']')?.parse().ok()?;
    Some((program_id, depth))
}

fn parse_exit(log: &str) -> Option<&str> {
    let suffix = log.strip_prefix("Program ")?;
    suffix.strip_suffix(" success").or_else(|| {
        suffix
            .split_once(" failed:")
            .map(|(program_id, _)| program_id)
    })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use futures_util::stream;
    use soldisco_domain::{Commitment, Network};
    use soldisco_solana_rpc::{
        AccountRecord, ProgramLogNotification, ReadContext, RpcError, RpcHealth, SignaturePage,
        SignaturePageRequest, SolanaReader, TransactionRecord,
    };
    use soldisco_source_pump::PumpProgram;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    use super::{
        CollectorError, CollectorRuntimeConfig, LiveFetchHealth, LiveProcessingContext,
        LiveSourceExit, LogScopeError, ReceivedProgramLogNotification, SourceConnectionState,
        fetch_live_batch, next_retry_delay, process_live_notifications, scope_program_data_logs,
    };

    const PUMP: &str = "pump";
    const OTHER: &str = "other";

    #[test]
    fn scopes_nested_events_to_their_top_level_instruction() {
        let logs = vec![
            format!("Program {OTHER} invoke [1]"),
            format!("Program {PUMP} invoke [2]"),
            "Program data: first".to_owned(),
            format!("Program {PUMP} success"),
            format!("Program {OTHER} success"),
            format!("Program {PUMP} invoke [1]"),
            "Program data: second".to_owned(),
            "Program data: third".to_owned(),
            format!("Program {PUMP} success"),
        ];

        let scoped = scope_program_data_logs(PUMP, 42, None, "signature", &logs)
            .expect("valid execution logs");

        assert_eq!(scoped.len(), 3);
        assert_eq!(scoped[0].coordinate.instruction_index, 0);
        assert_eq!(scoped[0].coordinate.event_index, 0);
        assert_eq!(scoped[1].coordinate.instruction_index, 1);
        assert_eq!(scoped[1].coordinate.event_index, 0);
        assert_eq!(scoped[2].coordinate.instruction_index, 1);
        assert_eq!(scoped[2].coordinate.event_index, 1);
    }

    #[test]
    fn source_transaction_index_is_preserved_in_scoped_coordinates() {
        let logs = vec![
            format!("Program {PUMP} invoke [1]"),
            "Program data: first".to_owned(),
            format!("Program {PUMP} success"),
        ];

        let scoped = scope_program_data_logs(PUMP, 42, Some(9), "signature", &logs)
            .expect("valid execution logs");

        assert_eq!(scoped[0].coordinate.transaction_index, Some(9));
    }

    #[test]
    fn ignores_program_data_from_other_programs() {
        let logs = vec![
            format!("Program {OTHER} invoke [1]"),
            "Program data: unrelated".to_owned(),
            format!("Program {OTHER} success"),
        ];

        assert!(
            scope_program_data_logs(PUMP, 1, None, "signature", &logs)
                .expect("valid logs")
                .is_empty()
        );
    }

    #[test]
    fn rejects_impossible_invocation_depth() {
        let error = scope_program_data_logs(
            PUMP,
            1,
            None,
            "signature",
            &[format!("Program {PUMP} invoke [2]")],
        )
        .expect_err("depth two requires a parent");

        assert_eq!(error, LogScopeError::InvalidInvocationDepth { depth: 2 });
    }

    #[test]
    fn rpc_retry_backoff_is_bounded() {
        assert_eq!(
            next_retry_delay(Duration::from_secs(1), Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        assert_eq!(
            next_retry_delay(Duration::from_secs(20), Duration::from_secs(1)),
            Duration::from_secs(30)
        );
        assert_eq!(
            next_retry_delay(Duration::from_secs(40), Duration::from_secs(40)),
            Duration::from_secs(40)
        );
    }

    #[derive(Default)]
    struct FakeReader {
        active: AtomicUsize,
        maximum_active: AtomicUsize,
        attempts: Mutex<BTreeMap<String, usize>>,
    }

    impl FakeReader {
        fn attempts(&self, signature: &str) -> usize {
            *self
                .attempts
                .lock()
                .expect("attempt lock")
                .get(signature)
                .unwrap_or(&0)
        }
    }

    #[async_trait::async_trait]
    impl SolanaReader for FakeReader {
        async fn health(&self) -> RpcHealth {
            RpcHealth::Up
        }

        async fn transaction(
            &self,
            signature: &str,
            _context: ReadContext,
        ) -> Result<Option<TransactionRecord>, RpcError> {
            let attempt = {
                let mut attempts = self.attempts.lock().expect("attempt lock");
                let attempt = attempts.entry(signature.to_owned()).or_default();
                *attempt += 1;
                *attempt
            };
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum_active.fetch_max(active, Ordering::SeqCst);
            let delay = if signature == "slow" { 40 } else { 2 };
            tokio::time::sleep(Duration::from_millis(delay)).await;
            self.active.fetch_sub(1, Ordering::SeqCst);

            if (signature == "retry" && attempt < 3) || signature == "missing" {
                return Ok(None);
            }
            Ok(Some(TransactionRecord {
                slot: 42,
                transaction_index: Some(
                    u64::try_from(attempt).expect("small fake attempt fits in u64"),
                ),
                signature: signature.to_owned(),
                block_time_unix_seconds: None,
                instructions: Vec::new(),
                log_messages: Vec::new(),
                transaction_error: (signature == "failed").then(|| "{}".to_owned()),
            }))
        }

        async fn account(
            &self,
            _address: &str,
            _context: ReadContext,
        ) -> Result<Option<AccountRecord>, RpcError> {
            Ok(None)
        }

        async fn signature_page(
            &self,
            _request: SignaturePageRequest,
        ) -> Result<SignaturePage, RpcError> {
            Err(RpcError::InvalidRequest(
                "unused by collector unit test".to_owned(),
            ))
        }
    }

    fn runtime_config() -> CollectorRuntimeConfig {
        CollectorRuntimeConfig {
            network: Network::SolanaMainnet,
            commitment: Commitment::Confirmed,
            reconnect_delay: Duration::from_millis(1),
            request_timeout: Duration::from_secs(1),
            rpc_max_in_flight: 3,
            live_fetch_max_attempts: 3,
            subscription_idle_timeout: Duration::from_secs(30),
            recovery_page_size: 100,
            recovery_max_records: 5_000,
        }
    }

    fn received(signature: &str, received_time_unix_ms: i64) -> ReceivedProgramLogNotification {
        ReceivedProgramLogNotification {
            notification: ProgramLogNotification {
                subscription_id: 1,
                slot: 42,
                signature: signature.to_owned(),
                log_messages: Vec::new(),
                transaction_error: None,
            },
            received_time_unix_ms,
        }
    }

    fn failed_received(
        signature: &str,
        received_time_unix_ms: i64,
    ) -> ReceivedProgramLogNotification {
        let mut received = received(signature, received_time_unix_ms);
        received.notification.transaction_error = Some("{}".to_owned());
        received
    }

    #[tokio::test]
    async fn live_fetches_are_concurrent_but_batches_remain_in_notification_order() {
        let reader = Arc::new(FakeReader::default());
        let (sender, mut receiver) = mpsc::channel(3);
        let (connections, _connection_receiver) = mpsc::unbounded_channel();
        let notifications = stream::iter([
            Ok(received("slow", 10)),
            Ok(received("fast", 11)),
            Ok(received("third", 12)),
        ]);

        let exit = process_live_notifications(
            PumpProgram::Pump,
            notifications,
            LiveProcessingContext {
                http: reader.clone(),
                batches: sender,
                connections,
                cancellation: CancellationToken::new(),
                config: runtime_config(),
                ready_state: SourceConnectionState::Ready,
            },
        )
        .await
        .expect("ordered live processing");

        assert!(matches!(exit, LiveSourceExit::Ended));
        assert!(
            reader.maximum_active.load(Ordering::SeqCst) >= 2,
            "more than one authoritative fetch should run at once"
        );
        let batches = [
            receiver.recv().await.expect("slow batch"),
            receiver.recv().await.expect("fast batch"),
            receiver.recv().await.expect("third batch"),
        ];
        assert_eq!(
            batches.map(|batch| batch.signature),
            ["slow", "fast", "third"]
        );
    }

    #[tokio::test]
    async fn live_fetch_retry_is_finite_and_preserves_source_receipt_time() {
        let reader = FakeReader::default();
        let (connections, mut connection_updates) = mpsc::unbounded_channel();
        let health = LiveFetchHealth::new(
            connections,
            PumpProgram::Pump.source_program(),
            SourceConnectionState::Ready,
        );
        let batch = fetch_live_batch(
            &reader,
            PumpProgram::Pump,
            received("retry", 123_456),
            &runtime_config(),
            Some(&health),
        )
        .await
        .expect("third attempt should succeed");

        assert_eq!(reader.attempts("retry"), 3);
        assert_eq!(batch.received_time_unix_ms, 123_456);
        assert_eq!(
            connection_updates
                .recv()
                .await
                .expect("retrying status")
                .state,
            SourceConnectionState::Recovering
        );
        assert_eq!(
            connection_updates
                .recv()
                .await
                .expect("recovered status")
                .state,
            SourceConnectionState::Ready
        );

        let mut config = runtime_config();
        config.live_fetch_max_attempts = 2;
        let error = fetch_live_batch(
            &reader,
            PumpProgram::Pump,
            received("missing", 123_456),
            &config,
            None,
        )
        .await
        .expect_err("missing transaction must exhaust its finite budget");

        assert!(matches!(
            error,
            CollectorError::LiveFetchExhausted { attempts: 2, .. }
        ));
        assert_eq!(reader.attempts("missing"), 2);
    }

    #[tokio::test]
    async fn failed_websocket_notifications_are_still_verified_over_http() {
        let reader = FakeReader::default();
        let batch = fetch_live_batch(
            &reader,
            PumpProgram::Pump,
            failed_received("failed", 123_456),
            &runtime_config(),
            None,
        )
        .await
        .expect("matching authoritative failed transaction");

        assert_eq!(reader.attempts("failed"), 1);
        assert!(batch.transaction_error.is_some());
        assert_eq!(batch.signature, "failed");
    }

    #[tokio::test]
    async fn retry_recovery_does_not_hide_a_persisted_history_gap() {
        let (connections, mut updates) = mpsc::unbounded_channel();
        let health = LiveFetchHealth::new(
            connections,
            PumpProgram::Pump.source_program(),
            SourceConnectionState::ReadyWithGap,
        );

        let guard = health.begin_retry();
        guard.recovered();

        assert_eq!(
            updates.recv().await.expect("recovering update").state,
            SourceConnectionState::Recovering
        );
        assert_eq!(
            updates.recv().await.expect("gapped-ready update").state,
            SourceConnectionState::ReadyWithGap
        );
    }
}
