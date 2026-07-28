use std::{
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use futures_util::{Stream, StreamExt as _, stream};
use soldisco_discovery_engine::{
    ObservationWindowProvision, ObservationWindowRegistry, ObservationWindowToken,
};
use soldisco_domain::{Commitment, Network, SourceProgram};
use soldisco_solana_rpc::{
    ProgramLogNotification, ReadContext, RpcError, SolanaHttpClient, SolanaPubsubClient,
    SolanaReader, TransactionInstructionRecord, TransactionRecord,
};
use soldisco_source_pump::PumpProgram;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::{
    discovery_rpc::DiscoveryRpcGate,
    intake::{BatchPurpose, NotificationDisposition, classify_notification},
};

const MAX_PUBSUB_RECONNECT_DELAY: Duration = Duration::from_secs(30);

#[derive(Clone, Debug)]
pub struct CollectorRuntimeConfig {
    pub network: Network,
    pub commitment: Commitment,
    pub reconnect_delay: Duration,
    pub request_timeout: Duration,
    pub rpc_max_in_flight: usize,
    pub notification_processing_capacity: usize,
    pub subscription_idle_timeout: Duration,
    pub maximum_discovery_age: Duration,
    pub observation_window_duration: Duration,
}

#[derive(Clone)]
pub struct ProgramSourceContext {
    pub http: Arc<SolanaHttpClient>,
    pub pubsub: SolanaPubsubClient,
    pub batches: mpsc::Sender<ProgramLogBatch>,
    pub connections: mpsc::UnboundedSender<SourceConnectionUpdate>,
    pub discovery_rpc_health: mpsc::UnboundedSender<DiscoveryRpcHealthUpdate>,
    pub windows: ObservationWindowRegistry,
    pub discovery_rpc: DiscoveryRpcGate,
    pub config: CollectorRuntimeConfig,
}

#[derive(Clone, Debug)]
pub struct ProgramLogBatch {
    pub purpose: BatchPurpose,
    pub slot: u64,
    pub transaction_index: Option<u64>,
    pub signature: String,
    /// Wall-clock time at which this source record was admitted from the
    /// subscription into available collector work capacity, before any
    /// authoritative HTTP fetch or discovery-RPC admission wait.
    pub received_time_unix_ms: i64,
    pub instructions: Vec<TransactionInstructionRecord>,
    pub log_messages: Vec<String>,
    pub transaction_error: Option<String>,
    pub window_tokens: Vec<ObservationWindowToken>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceConnectionState {
    Connecting,
    Ready,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceConnectionUpdate {
    pub source_program: SourceProgram,
    pub state: SourceConnectionState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryRpcHealthUpdate {
    Failed,
    RateLimited,
    Successful,
}

#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("collector processing queue is closed")]
    ProcessingQueueClosed,
}

#[derive(Clone, Debug)]
struct ReceivedProgramLogNotification {
    notification: ProgramLogNotification,
    received_time_unix_ms: i64,
}

#[derive(Debug)]
enum LiveSourceExit {
    Subscription(RpcError),
    Ended,
}

enum LiveNotificationOutcome {
    Batches(Vec<ProgramLogBatch>),
    Skipped,
    Subscription(RpcError),
}

struct LiveProcessingContext<R> {
    http: Arc<R>,
    batches: mpsc::Sender<ProgramLogBatch>,
    cancellation: CancellationToken,
    config: CollectorRuntimeConfig,
    windows: ObservationWindowRegistry,
    discovery_rpc: DiscoveryRpcGate,
    discovery_rpc_health: mpsc::UnboundedSender<DiscoveryRpcHealthUpdate>,
}

struct ProvisionalWindowGuard {
    provisions: Vec<ObservationWindowProvision>,
    armed: bool,
}

impl ProvisionalWindowGuard {
    fn new(provisions: Vec<ObservationWindowProvision>) -> Self {
        Self {
            provisions,
            armed: true,
        }
    }

    fn has_new_open_token_at(&self, observed_at_unix_ms: i64) -> bool {
        self.provisions.iter().any(|provision| {
            provision.newly_opened && provision.token.is_open_at(observed_at_unix_ms)
        })
    }

    fn into_new_tokens(mut self) -> Vec<ObservationWindowToken> {
        self.armed = false;
        std::mem::take(&mut self.provisions)
            .into_iter()
            .filter(|provision| provision.newly_opened)
            .map(|provision| provision.token)
            .collect()
    }
}

impl Drop for ProvisionalWindowGuard {
    fn drop(&mut self) {
        if self.armed {
            cancel_new_provisions(&self.provisions);
        }
    }
}

pub async fn run_program_source(
    program: PumpProgram,
    context: ProgramSourceContext,
    cancellation: CancellationToken,
) -> Result<(), CollectorError> {
    let source_program = program.source_program();
    let mut reconnect_delay = context.config.reconnect_delay;
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
                wait_to_reconnect(&cancellation, reconnect_delay).await?;
                reconnect_delay =
                    next_reconnect_delay(reconnect_delay, context.config.reconnect_delay);
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
                wait_to_reconnect(&cancellation, reconnect_delay).await?;
                reconnect_delay =
                    next_reconnect_delay(reconnect_delay, context.config.reconnect_delay);
                continue;
            }
        };

        let connected_at = tokio::time::Instant::now();
        send_connection(
            &context.connections,
            source_program,
            SourceConnectionState::Ready,
        );
        let notifications =
            live_notification_stream(subscription, context.config.subscription_idle_timeout);
        let exit = process_live_notifications(
            program,
            notifications,
            LiveProcessingContext {
                http: context.http.clone(),
                batches: context.batches.clone(),
                cancellation: cancellation.clone(),
                config: context.config.clone(),
                windows: context.windows.clone(),
                discovery_rpc: context.discovery_rpc.clone(),
                discovery_rpc_health: context.discovery_rpc_health.clone(),
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

        if connected_at.elapsed() >= context.config.subscription_idle_timeout {
            reconnect_delay = context.config.reconnect_delay;
        }
        wait_to_reconnect(&cancellation, reconnect_delay).await?;
        reconnect_delay = next_reconnect_delay(reconnect_delay, context.config.reconnect_delay);
    }
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
    let notification_processing_capacity = context.config.notification_processing_capacity;
    let fetches = notifications
        .map(|received| {
            let http = context.http.clone();
            let config = context.config.clone();
            let windows = context.windows.clone();
            let discovery_rpc = context.discovery_rpc.clone();
            let discovery_rpc_health = context.discovery_rpc_health.clone();
            async move {
                match received {
                    Ok(received) => {
                        collect_live_notification(
                            http.as_ref(),
                            program,
                            received,
                            &config,
                            &windows,
                            &discovery_rpc,
                            &discovery_rpc_health,
                        )
                        .await
                    }
                    Err(error) => LiveNotificationOutcome::Subscription(error),
                }
            }
        })
        .buffer_unordered(notification_processing_capacity);
    futures_util::pin_mut!(fetches);

    loop {
        let fetched = tokio::select! {
            () = context.cancellation.cancelled() => return Ok(LiveSourceExit::Ended),
            fetched = fetches.next() => fetched,
        };
        match fetched {
            Some(LiveNotificationOutcome::Batches(batches)) => {
                for batch in batches {
                    send_raw_batch(&context.batches, batch, &context.cancellation).await?;
                }
            }
            Some(LiveNotificationOutcome::Skipped) => {}
            Some(LiveNotificationOutcome::Subscription(error)) => {
                return Ok(LiveSourceExit::Subscription(error));
            }
            None => return Ok(LiveSourceExit::Ended),
        }
    }
}

async fn collect_live_notification<R: SolanaReader + ?Sized>(
    http: &R,
    subscription_program: PumpProgram,
    received: ReceivedProgramLogNotification,
    config: &CollectorRuntimeConfig,
    windows: &ObservationWindowRegistry,
    discovery_rpc: &DiscoveryRpcGate,
    discovery_rpc_health: &mpsc::UnboundedSender<DiscoveryRpcHealthUpdate>,
) -> LiveNotificationOutcome {
    let disposition = classify_notification(
        &received.notification,
        windows,
        config.maximum_discovery_age,
        received.received_time_unix_ms,
        unix_time_millis(),
    );
    let (discovery_seeds, tracked_windows) = match disposition {
        NotificationDisposition::Drop(reason) => {
            tracing::trace!(
                source = ?subscription_program.source_program(),
                signature = %received.notification.signature,
                ?reason,
                "discarded live program notification before HTTP"
            );
            return LiveNotificationOutcome::Skipped;
        }
        NotificationDisposition::CollectTrackedActivity(window_tokens) => {
            return LiveNotificationOutcome::Batches(vec![notification_batch(
                received,
                BatchPurpose::TrackedActivity,
                window_tokens,
            )]);
        }
        NotificationDisposition::FetchDiscovery {
            seeds,
            tracked_windows,
        } => (seeds, tracked_windows),
    };

    if !discovery_rpc.claim_signature(&received.notification.signature) {
        return tracked_activity_or_skip(received, tracked_windows);
    }

    let admission_timeout = discovery_admission_timeout(
        &discovery_seeds,
        config.maximum_discovery_age,
        received.received_time_unix_ms,
        unix_time_millis(),
    );
    let guard = ProvisionalWindowGuard::new(
        discovery_seeds
            .into_iter()
            .map(|seed| {
                windows.provision_target(
                    seed.target,
                    seed.mint,
                    received.received_time_unix_ms,
                    config.observation_window_duration,
                )
            })
            .collect::<Vec<_>>(),
    );

    let Some(admission_timeout) = admission_timeout else {
        return tracked_activity_or_skip(received, tracked_windows);
    };
    if !guard.has_new_open_token_at(received.received_time_unix_ms) {
        return tracked_activity_or_skip(received, tracked_windows);
    }
    let Ok(Ok(_permit)) = tokio::time::timeout(admission_timeout, discovery_rpc.acquire()).await
    else {
        return tracked_activity_or_skip(received, tracked_windows);
    };
    let disposition = classify_notification(
        &received.notification,
        windows,
        config.maximum_discovery_age,
        received.received_time_unix_ms,
        unix_time_millis(),
    );
    if !matches!(disposition, NotificationDisposition::FetchDiscovery { .. })
        || !guard.has_new_open_token_at(received.received_time_unix_ms)
    {
        return tracked_activity_or_skip(received, tracked_windows);
    }

    let notification = &received.notification;
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
    let transaction = match result {
        Ok(Ok(Some(transaction))) => transaction,
        Ok(Ok(None)) => {
            let _ = discovery_rpc_health.send(DiscoveryRpcHealthUpdate::Failed);
            tracing::debug!(
                source = ?subscription_program.source_program(),
                signature = %notification.signature,
                "one-shot discovery transaction was unavailable; skipping"
            );
            return tracked_activity_or_skip(received, tracked_windows);
        }
        Ok(Err(error)) => {
            if matches!(error, RpcError::RateLimited) {
                discovery_rpc.record_rate_limit().await;
                let _ = discovery_rpc_health.send(DiscoveryRpcHealthUpdate::RateLimited);
                tracing::warn!(
                    source = ?subscription_program.source_program(),
                    signature = %notification.signature,
                    cooldown_ms = discovery_rpc.rate_limit_cooldown().as_millis(),
                    "one-shot discovery transaction was rate limited; skipping this signature"
                );
            } else {
                let _ = discovery_rpc_health.send(DiscoveryRpcHealthUpdate::Failed);
                tracing::debug!(
                    source = ?subscription_program.source_program(),
                    signature = %notification.signature,
                    %error,
                    "one-shot discovery transaction fetch failed; skipping"
                );
            }
            return tracked_activity_or_skip(received, tracked_windows);
        }
        Err(_) => {
            let _ = discovery_rpc_health.send(DiscoveryRpcHealthUpdate::Failed);
            tracing::debug!(
                source = ?subscription_program.source_program(),
                signature = %notification.signature,
                "one-shot discovery transaction fetch timed out; skipping"
            );
            return tracked_activity_or_skip(received, tracked_windows);
        }
    };

    if transaction.slot != notification.slot
        || transaction.signature != notification.signature
        || !transaction.succeeded()
    {
        let _ = discovery_rpc_health.send(DiscoveryRpcHealthUpdate::Failed);
        tracing::warn!(
            source = ?subscription_program.source_program(),
            signature = %notification.signature,
            "one-shot discovery transaction did not match its successful WebSocket notification; skipping"
        );
        return tracked_activity_or_skip(received, tracked_windows);
    }
    let _ = discovery_rpc_health.send(DiscoveryRpcHealthUpdate::Successful);
    let discovery_tokens = guard.into_new_tokens();
    let mut batches = vec![transaction_batch(
        transaction.clone(),
        received.received_time_unix_ms,
        BatchPurpose::Discovery,
        discovery_tokens,
    )];
    if !tracked_windows.is_empty() {
        batches.push(transaction_batch(
            transaction,
            received.received_time_unix_ms,
            BatchPurpose::TrackedActivity,
            tracked_windows,
        ));
    }
    LiveNotificationOutcome::Batches(batches)
}

fn discovery_admission_timeout(
    seeds: &[super::intake::DiscoverySeed],
    maximum_discovery_age: Duration,
    received_time_unix_ms: i64,
    now_unix_ms: i64,
) -> Option<Duration> {
    let maximum_age_ms = i64::try_from(maximum_discovery_age.as_millis()).unwrap_or(i64::MAX);
    let source_deadline_unix_ms = seeds
        .iter()
        .map(|seed| {
            seed.source_event_time_unix_ms
                .saturating_add(maximum_age_ms)
        })
        .max()?;
    let receipt_deadline_unix_ms = received_time_unix_ms.saturating_add(maximum_age_ms);
    let deadline_unix_ms = source_deadline_unix_ms.min(receipt_deadline_unix_ms);
    let remaining_ms = deadline_unix_ms.checked_sub(now_unix_ms)?;
    let remaining_ms = u64::try_from(remaining_ms).ok()?;
    (remaining_ms > 0).then(|| Duration::from_millis(remaining_ms))
}

fn tracked_activity_or_skip(
    received: ReceivedProgramLogNotification,
    window_tokens: Vec<ObservationWindowToken>,
) -> LiveNotificationOutcome {
    if window_tokens.is_empty() {
        LiveNotificationOutcome::Skipped
    } else {
        LiveNotificationOutcome::Batches(vec![notification_batch(
            received,
            BatchPurpose::TrackedActivity,
            window_tokens,
        )])
    }
}

fn cancel_new_provisions(provisions: &[ObservationWindowProvision]) {
    for provision in provisions.iter().filter(|provision| provision.newly_opened) {
        provision.token.cancel_if_pending();
    }
}

fn transaction_batch(
    transaction: TransactionRecord,
    received_time_unix_ms: i64,
    purpose: BatchPurpose,
    window_tokens: Vec<ObservationWindowToken>,
) -> ProgramLogBatch {
    ProgramLogBatch {
        purpose,
        slot: transaction.slot,
        transaction_index: transaction.transaction_index,
        signature: transaction.signature,
        received_time_unix_ms,
        instructions: transaction.instructions,
        log_messages: transaction.log_messages,
        transaction_error: transaction.transaction_error,
        window_tokens,
    }
}

fn notification_batch(
    received: ReceivedProgramLogNotification,
    purpose: BatchPurpose,
    window_tokens: Vec<ObservationWindowToken>,
) -> ProgramLogBatch {
    ProgramLogBatch {
        purpose,
        slot: received.notification.slot,
        transaction_index: None,
        signature: received.notification.signature,
        received_time_unix_ms: received.received_time_unix_ms,
        instructions: Vec::new(),
        log_messages: received.notification.log_messages,
        transaction_error: received.notification.transaction_error,
        window_tokens,
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

fn next_reconnect_delay(current: Duration, configured_minimum: Duration) -> Duration {
    current
        .saturating_mul(2)
        .min(MAX_PUBSUB_RECONNECT_DELAY.max(configured_minimum))
}

fn unix_time_millis() -> i64 {
    let milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    i64::try_from(milliseconds).unwrap_or(i64::MAX)
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

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use futures_util::stream;
    use soldisco_discovery_engine::{ObservationWindowRegistry, ObservationWindowStatus};
    use soldisco_domain::{ChainCoordinate, Commitment, Network};
    use soldisco_solana_rpc::{
        AccountRecord, ProgramLogNotification, ReadContext, RpcError, RpcHealth, SignaturePage,
        SignaturePageRequest, SolanaReader, TransactionRecord,
    };
    use soldisco_source_pump::{
        COMPLETE_EVENT_DISCRIMINATOR, CREATE_EVENT_DISCRIMINATOR, PUMP_PROGRAM_ID, PumpProgram,
        decode_anchor_event,
    };
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    use super::{
        CollectorRuntimeConfig, LiveNotificationOutcome, LiveProcessingContext, LiveSourceExit,
        ReceivedProgramLogNotification, collect_live_notification, next_reconnect_delay,
        process_live_notifications,
    };
    use crate::jobs::{
        discovery_rpc::DiscoveryRpcGate,
        intake::{BatchPurpose, discovery_seed, event_tracking_target},
    };

    #[test]
    fn pubsub_reconnect_backoff_is_bounded() {
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(1), Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(20), Duration::from_secs(1)),
            Duration::from_secs(30)
        );
        assert_eq!(
            next_reconnect_delay(Duration::from_secs(40), Duration::from_secs(40)),
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

            if signature == "missing" {
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
            notification_processing_capacity: 32,
            subscription_idle_timeout: Duration::from_secs(30),
            maximum_discovery_age: Duration::from_secs(30),
            observation_window_duration: Duration::from_secs(5),
        }
    }

    fn discovery_rpc(maximum_in_flight: usize) -> DiscoveryRpcGate {
        DiscoveryRpcGate::new(
            maximum_in_flight,
            1_000,
            Duration::from_millis(10),
            Duration::from_secs(30),
        )
    }

    fn push_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend(
            u32::try_from(value.len())
                .expect("fixture string fits in u32")
                .to_le_bytes(),
        );
        bytes.extend(value.as_bytes());
    }

    fn push_pubkey(bytes: &mut Vec<u8>, marker: u8) {
        bytes.extend([marker; 32]);
    }

    fn create_event(timestamp: i64) -> Vec<u8> {
        create_event_for_mint(timestamp, 1)
    }

    fn create_event_for_mint(timestamp: i64, mint_marker: u8) -> Vec<u8> {
        let mut bytes = CREATE_EVENT_DISCRIMINATOR.to_vec();
        push_string(&mut bytes, "Fixture Coin");
        push_string(&mut bytes, "FIX");
        push_string(&mut bytes, "https://example.invalid/fixture.json");
        for marker in [mint_marker, 2, 3, 4] {
            push_pubkey(&mut bytes, marker);
        }
        bytes.extend(timestamp.to_le_bytes());
        for value in 1_u64..=4 {
            bytes.extend((value * 1_000).to_le_bytes());
        }
        push_pubkey(&mut bytes, 5);
        bytes.push(0);
        bytes.push(0);
        push_pubkey(&mut bytes, 6);
        bytes.extend(5_000_u64.to_le_bytes());
        bytes
    }

    fn complete_event(timestamp: i64, mint_marker: u8) -> Vec<u8> {
        let mut bytes = COMPLETE_EVENT_DISCRIMINATOR.to_vec();
        push_pubkey(&mut bytes, 8);
        push_pubkey(&mut bytes, mint_marker);
        push_pubkey(&mut bytes, 7);
        bytes.extend(timestamp.to_le_bytes());
        push_pubkey(&mut bytes, 6);
        bytes
    }

    fn received(signature: &str, received_time_unix_ms: i64) -> ReceivedProgramLogNotification {
        received_with_event(
            signature,
            received_time_unix_ms,
            create_event_for_mint(
                super::unix_time_millis() / 1_000,
                signature.bytes().next().unwrap_or(1),
            ),
        )
    }

    fn received_with_event(
        signature: &str,
        received_time_unix_ms: i64,
        event: Vec<u8>,
    ) -> ReceivedProgramLogNotification {
        received_with_events(signature, received_time_unix_ms, &[event])
    }

    fn received_with_events(
        signature: &str,
        received_time_unix_ms: i64,
        events: &[Vec<u8>],
    ) -> ReceivedProgramLogNotification {
        let mut log_messages = vec![format!("Program {PUMP_PROGRAM_ID} invoke [1]")];
        log_messages.extend(
            events
                .iter()
                .map(|event| format!("Program data: {}", STANDARD.encode(event))),
        );
        log_messages.push(format!("Program {PUMP_PROGRAM_ID} success"));
        ReceivedProgramLogNotification {
            notification: ProgramLogNotification {
                subscription_id: 1,
                slot: 42,
                signature: signature.to_owned(),
                log_messages,
                transaction_error: None,
            },
            received_time_unix_ms,
        }
    }

    fn failed_received(signature: &str) -> ReceivedProgramLogNotification {
        let received_time_unix_ms = super::unix_time_millis();
        let mut received = received(signature, received_time_unix_ms);
        received.notification.transaction_error = Some("{}".to_owned());
        received
    }

    #[tokio::test]
    async fn live_reader_continues_while_discovery_fetches_are_slow() {
        let reader = Arc::new(FakeReader::default());
        let (sender, mut receiver) = mpsc::channel(3);
        let now = super::unix_time_millis();
        let notifications = stream::iter([
            Ok(received("slow", now)),
            Ok(received("fast", now)),
            Ok(received("third", now)),
        ]);

        let exit = process_live_notifications(
            PumpProgram::Pump,
            notifications,
            LiveProcessingContext {
                http: reader.clone(),
                batches: sender,
                cancellation: CancellationToken::new(),
                config: runtime_config(),
                windows: ObservationWindowRegistry::new(8),
                discovery_rpc: discovery_rpc(3),
                discovery_rpc_health: mpsc::unbounded_channel().0,
            },
        )
        .await
        .expect("ordered live processing");

        assert!(matches!(exit, LiveSourceExit::Ended));
        assert!(
            reader.maximum_active.load(Ordering::SeqCst) >= 2,
            "more than one authoritative fetch should run at once"
        );
        let mut batches = [
            receiver.recv().await.expect("slow batch"),
            receiver.recv().await.expect("fast batch"),
            receiver.recv().await.expect("third batch"),
        ];
        assert_ne!(
            batches[0].signature, "slow",
            "a slow discovery fetch must not block later notification work"
        );
        batches.sort_by(|left, right| left.signature.cmp(&right.signature));
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.signature.as_str())
                .collect::<Vec<_>>(),
            ["fast", "slow", "third"]
        );
        assert!(
            batches
                .iter()
                .all(|batch| batch.purpose == super::BatchPurpose::Discovery)
        );
    }

    #[tokio::test]
    async fn unavailable_discovery_is_attempted_once_and_the_stream_continues() {
        let reader = Arc::new(FakeReader::default());
        let (sender, mut receiver) = mpsc::channel(2);
        let now = super::unix_time_millis();
        let notifications =
            stream::iter([Ok(received("missing", now)), Ok(received("available", now))]);

        let exit = process_live_notifications(
            PumpProgram::Pump,
            notifications,
            LiveProcessingContext {
                http: reader.clone(),
                batches: sender,
                cancellation: CancellationToken::new(),
                config: runtime_config(),
                windows: ObservationWindowRegistry::new(8),
                discovery_rpc: discovery_rpc(2),
                discovery_rpc_health: mpsc::unbounded_channel().0,
            },
        )
        .await
        .expect("one failed transaction must not end the stream");

        assert!(matches!(exit, LiveSourceExit::Ended));
        assert_eq!(reader.attempts("missing"), 1);
        assert_eq!(reader.attempts("available"), 1);
        assert_eq!(
            receiver.recv().await.expect("available batch").signature,
            "available"
        );
        assert!(receiver.try_recv().is_err());
    }

    #[tokio::test]
    async fn failed_websocket_notifications_are_dropped_without_http() {
        let reader = FakeReader::default();
        let outcome = collect_live_notification(
            &reader,
            PumpProgram::Pump,
            failed_received("failed"),
            &runtime_config(),
            &ObservationWindowRegistry::new(8),
            &discovery_rpc(1),
            &mpsc::unbounded_channel().0,
        )
        .await;

        assert!(matches!(outcome, LiveNotificationOutcome::Skipped));
        assert_eq!(reader.attempts("failed"), 0);
    }

    #[tokio::test]
    async fn stale_and_irrelevant_notifications_never_use_http() {
        let reader = FakeReader::default();
        let config = runtime_config();
        let windows = ObservationWindowRegistry::new(8);
        let discovery_rpc = discovery_rpc(1);
        let (health, _health_receiver) = mpsc::unbounded_channel();
        let now = super::unix_time_millis();
        let stale =
            received_with_event("stale", now, create_event((now / 1_000).saturating_sub(60)));
        let irrelevant = ReceivedProgramLogNotification {
            notification: ProgramLogNotification {
                subscription_id: 1,
                slot: 42,
                signature: "irrelevant".to_owned(),
                log_messages: vec![format!("Program {PUMP_PROGRAM_ID} invoke [1]")],
                transaction_error: None,
            },
            received_time_unix_ms: now,
        };

        let stale = collect_live_notification(
            &reader,
            PumpProgram::Pump,
            stale,
            &config,
            &windows,
            &discovery_rpc,
            &health,
        )
        .await;
        let irrelevant = collect_live_notification(
            &reader,
            PumpProgram::Pump,
            irrelevant,
            &config,
            &windows,
            &discovery_rpc,
            &health,
        )
        .await;

        assert!(matches!(stale, LiveNotificationOutcome::Skipped));
        assert!(matches!(irrelevant, LiveNotificationOutcome::Skipped));
        assert_eq!(reader.attempts("stale"), 0);
        assert_eq!(reader.attempts("irrelevant"), 0);
    }

    #[tokio::test]
    async fn duplicate_program_subscriptions_share_one_discovery_attempt() {
        let reader = Arc::new(FakeReader::default());
        let config = runtime_config();
        let windows = ObservationWindowRegistry::new(8);
        let discovery_rpc = discovery_rpc(2);
        let (health, _health_receiver) = mpsc::unbounded_channel();
        let now = super::unix_time_millis();

        let pump = collect_live_notification(
            reader.as_ref(),
            PumpProgram::Pump,
            received("shared", now),
            &config,
            &windows,
            &discovery_rpc,
            &health,
        );
        let pump_swap = collect_live_notification(
            reader.as_ref(),
            PumpProgram::PumpSwap,
            received("shared", now),
            &config,
            &windows,
            &discovery_rpc,
            &health,
        );
        let (pump, pump_swap) = tokio::join!(pump, pump_swap);

        assert_eq!(reader.attempts("shared"), 1);
        assert_eq!(
            usize::from(matches!(pump, LiveNotificationOutcome::Batches(_)))
                + usize::from(matches!(pump_swap, LiveNotificationOutcome::Batches(_))),
            1
        );
    }

    #[tokio::test]
    async fn global_rpc_permits_bound_discovery_fetches() {
        let reader = Arc::new(FakeReader::default());
        let discovery_rpc = discovery_rpc(1);
        let windows = ObservationWindowRegistry::new(8);
        let config = runtime_config();
        let (health, _health_receiver) = mpsc::unbounded_channel();
        let now = super::unix_time_millis();
        let first = collect_live_notification(
            reader.as_ref(),
            PumpProgram::Pump,
            received("slow", now),
            &config,
            &windows,
            &discovery_rpc,
            &health,
        );
        let second = collect_live_notification(
            reader.as_ref(),
            PumpProgram::Pump,
            received("fast", now),
            &config,
            &windows,
            &discovery_rpc,
            &health,
        );

        let (first, second) = tokio::join!(first, second);

        assert!(matches!(first, LiveNotificationOutcome::Batches(_)));
        assert!(matches!(second, LiveNotificationOutcome::Batches(_)));
        assert_eq!(reader.maximum_active.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn mixed_discovery_keeps_unrelated_pending_activity_in_a_separate_batch() {
        let reader = FakeReader::default();
        let config = runtime_config();
        let windows = ObservationWindowRegistry::new(8);
        let discovery_rpc = discovery_rpc(1);
        let (health, _health_receiver) = mpsc::unbounded_channel();
        let now = super::unix_time_millis();
        let activity = complete_event(now / 1_000, 9);
        let decoded_activity = decode_anchor_event(
            PUMP_PROGRAM_ID,
            ChainCoordinate {
                slot: 42,
                transaction_index: None,
                signature: "mixed".to_owned(),
                instruction_index: 0,
                event_index: 1,
            },
            &activity,
        )
        .expect("complete fixture");
        let activity_target =
            event_tracking_target(&decoded_activity.event).expect("activity target");
        let pending_activity = windows.provision_target(
            activity_target,
            "pending-a".to_owned(),
            now,
            Duration::from_secs(5),
        );
        let notification = received_with_events(
            "mixed",
            now,
            &[create_event_for_mint(now / 1_000, 1), activity],
        );

        let outcome = collect_live_notification(
            &reader,
            PumpProgram::Pump,
            notification,
            &config,
            &windows,
            &discovery_rpc,
            &health,
        )
        .await;
        let LiveNotificationOutcome::Batches(batches) = outcome else {
            panic!("mixed discovery should produce authoritative batches");
        };

        assert_eq!(batches.len(), 2);
        let discovery = batches
            .iter()
            .find(|batch| batch.purpose == BatchPurpose::Discovery)
            .expect("discovery batch");
        let tracked = batches
            .iter()
            .find(|batch| batch.purpose == BatchPurpose::TrackedActivity)
            .expect("tracked-activity batch");
        assert_eq!(discovery.window_tokens.len(), 1);
        assert!(
            discovery
                .window_tokens
                .iter()
                .all(|token| token != &pending_activity.token)
        );
        assert_eq!(tracked.window_tokens, vec![pending_activity.token.clone()]);

        for token in &discovery.window_tokens {
            token.cancel_if_pending();
        }
        assert_eq!(
            pending_activity.token.status(),
            ObservationWindowStatus::Pending,
            "resolving discovery B must not cancel pending discovery A"
        );
    }

    #[tokio::test]
    async fn duplicate_discovery_delivery_preserves_newly_matched_activity_without_http() {
        let reader = FakeReader::default();
        let config = runtime_config();
        let windows = ObservationWindowRegistry::new(8);
        let discovery_rpc = discovery_rpc(1);
        let (health, _health_receiver) = mpsc::unbounded_channel();
        let now = super::unix_time_millis();
        let activity = complete_event(now / 1_000, 9);
        let decoded_activity = decode_anchor_event(
            PUMP_PROGRAM_ID,
            ChainCoordinate {
                slot: 42,
                transaction_index: None,
                signature: "duplicate".to_owned(),
                instruction_index: 0,
                event_index: 1,
            },
            &activity,
        )
        .expect("complete fixture");
        let activity_target =
            event_tracking_target(&decoded_activity.event).expect("activity target");
        let pending_activity = windows.provision_target(
            activity_target,
            "pending-a".to_owned(),
            now,
            Duration::from_secs(5),
        );
        assert!(discovery_rpc.claim_signature("duplicate"));
        let notification = received_with_events(
            "duplicate",
            now,
            &[create_event_for_mint(now / 1_000, 1), activity],
        );

        let outcome = collect_live_notification(
            &reader,
            PumpProgram::PumpSwap,
            notification,
            &config,
            &windows,
            &discovery_rpc,
            &health,
        )
        .await;
        let LiveNotificationOutcome::Batches(batches) = outcome else {
            panic!("duplicate delivery should preserve tracked activity");
        };

        assert_eq!(reader.attempts("duplicate"), 0);
        assert_eq!(batches.len(), 1);
        assert_eq!(batches[0].purpose, BatchPurpose::TrackedActivity);
        assert_eq!(
            batches[0].window_tokens,
            vec![pending_activity.token.clone()]
        );
        assert_eq!(
            pending_activity.token.status(),
            ObservationWindowStatus::Pending
        );
    }

    #[tokio::test]
    async fn replay_for_an_existing_provisional_target_uses_zero_http_and_cannot_cancel_its_owner()
    {
        let reader = FakeReader::default();
        let config = runtime_config();
        let windows = ObservationWindowRegistry::new(8);
        let discovery_rpc = discovery_rpc(1);
        let (health, _health_receiver) = mpsc::unbounded_channel();
        let now = super::unix_time_millis();
        let create = create_event_for_mint(now / 1_000, 1);
        let decoded = decode_anchor_event(
            PUMP_PROGRAM_ID,
            ChainCoordinate {
                slot: 42,
                transaction_index: None,
                signature: "original".to_owned(),
                instruction_index: 0,
                event_index: 0,
            },
            &create,
        )
        .expect("create fixture");
        let seed = discovery_seed(&decoded.event).expect("discovery seed");
        let original =
            windows.provision_target(seed.target, seed.mint, now, Duration::from_secs(5));

        let outcome = collect_live_notification(
            &reader,
            PumpProgram::Pump,
            received_with_event("replay", now, create),
            &config,
            &windows,
            &discovery_rpc,
            &health,
        )
        .await;

        assert!(matches!(outcome, LiveNotificationOutcome::Skipped));
        assert_eq!(reader.attempts("replay"), 0);
        assert_eq!(original.token.status(), ObservationWindowStatus::Pending);
    }

    #[tokio::test]
    async fn discovery_that_ages_out_waiting_for_admission_uses_zero_http_attempts() {
        let reader = FakeReader::default();
        let mut config = runtime_config();
        config.maximum_discovery_age = Duration::from_secs(1);
        let windows = ObservationWindowRegistry::new(8);
        let discovery_rpc = discovery_rpc(1);
        let (health, _health_receiver) = mpsc::unbounded_channel();
        let held_permit = discovery_rpc.acquire().await.expect("held permit");
        let now = super::unix_time_millis();
        let collection = collect_live_notification(
            &reader,
            PumpProgram::Pump,
            received_with_event(
                "aged-out",
                now,
                create_event((now / 1_000).saturating_add(1)),
            ),
            &config,
            &windows,
            &discovery_rpc,
            &health,
        );
        tokio::pin!(collection);
        tokio::select! {
            _ = &mut collection => panic!("blocked discovery should not resolve before its deadline"),
            () = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
        assert_eq!(
            windows.active_count(super::unix_time_millis()),
            1,
            "the test must reach provisional admission before waiting for expiry"
        );
        let outcome = tokio::time::timeout(Duration::from_secs(2), &mut collection)
            .await
            .expect("freshness deadline should bound admission wait");
        drop(held_permit);

        assert!(matches!(outcome, LiveNotificationOutcome::Skipped));
        assert_eq!(reader.attempts("aged-out"), 0);
        assert_eq!(windows.active_count(super::unix_time_millis()), 0);
    }
}
