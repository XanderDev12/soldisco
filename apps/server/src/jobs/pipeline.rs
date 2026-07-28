use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use soldisco_api_contracts::{PrefilterDefaults, StreamStatus};
use soldisco_discovery_engine::ObservationWindowRegistry;
use soldisco_domain::{Network, ObservationKey, SourceProgram, Venue};
use soldisco_persistence::{
    Database, IntakeQuarantineRecord, MAX_QUARANTINE_EVIDENCE_BASE64_BYTES, NewDiscoveryWindow,
    PersistenceError, PumpSwapPool, StoredPrefilterDefaults, WindowTargetKind,
};
use soldisco_solana_rpc::{RpcError, SolanaHttpClient, SolanaPubsubClient};
use soldisco_source_pump::{
    DECODER_VERSION, DecodeError, DecodedPumpEvent, PumpEvent, PumpProgram, decode_anchor_event,
    decode_cpi_event, decode_program_data_bytes, is_anchor_event_cpi,
    is_pinned_event_discriminator,
};
use thiserror::Error;
use tokio::{
    sync::{Notify, mpsc, watch},
    task::{JoinHandle, JoinSet},
};
use tokio_util::sync::CancellationToken;

use crate::{
    config::Config,
    jobs::{
        collector::{
            CollectorError, CollectorRuntimeConfig, DiscoveryRpcHealthUpdate, ProgramLogBatch,
            ProgramSourceContext, SourceConnectionState, run_program_source,
        },
        discovery::{DiscoveryWorkerConfig, run_discovery_worker},
        discovery_rpc::DiscoveryRpcGate,
        intake::{
            BatchPurpose, LogScopeError, PUMP_PROGRAMS, discovery_event_is_fresh, discovery_seed,
            event_matches_confirmed_window, is_discovery_event, scope_program_data_logs,
        },
        maintenance::{
            MaintenanceError, MaintenanceRuntimeConfig, ensure_storage_capacity, run_maintenance,
            run_maintenance_cycle,
        },
        normalization::{MarketRegistry, NormalizationError, normalize_pump_event},
        pending_activity::{PendingActivityAdmission, PendingActivityQueue},
        qualification::{QualificationWorkerError, run_qualification_finalizer},
    },
    state::LiveEventBus,
};

const PIPELINE_RESTART_INITIAL_DELAY: Duration = Duration::from_secs(1);
const PIPELINE_RESTART_MAX_DELAY: Duration = Duration::from_secs(30);
const MAX_QUARANTINE_RAW_BYTES: usize = (MAX_QUARANTINE_EVIDENCE_BASE64_BYTES / 4) * 3;
const SOLANA_MAINNET_GENESIS_HASH: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
const SOLANA_DEVNET_GENESIS_HASH: &str = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";
const DISCOVERY_RPC_FAILURES_BEFORE_DEGRADED: u32 = 3;
const DISCOVERY_SIGNATURE_RETENTION_MARGIN: Duration = Duration::from_secs(11);

#[derive(Clone)]
pub struct PipelineConfig {
    pub http_url: String,
    pub ws_url: String,
    pub collector: CollectorRuntimeConfig,
    pub maintenance: MaintenanceRuntimeConfig,
    pub queue_capacity: usize,
    pub maximum_active_windows: usize,
    pub discovery_rpc_requests_per_second: u32,
    pub discovery_rpc_rate_limit_cooldown: Duration,
    pub prefilter_revision: u64,
    pub prefilter_values: PrefilterDefaults,
    pub collector_run_id: String,
}

impl From<&Config> for PipelineConfig {
    fn from(config: &Config) -> Self {
        Self {
            http_url: config.solana_rpc_http_url.clone(),
            ws_url: config.solana_rpc_ws_url.clone(),
            collector: CollectorRuntimeConfig {
                network: config.solana_network,
                commitment: config.solana_commitment,
                reconnect_delay: config.solana_reconnect_delay,
                request_timeout: config.solana_request_timeout,
                rpc_max_in_flight: config.solana_rpc_max_in_flight,
                notification_processing_capacity: config.collector_queue_capacity,
                subscription_idle_timeout: config.solana_subscription_idle_timeout,
                maximum_discovery_age: config.discovery_max_event_age,
                observation_window_duration: config.discovery_observation_window,
            },
            maintenance: MaintenanceRuntimeConfig {
                terminal_history_retention: config.retention_terminal_history,
                projection_event_retention: config.retention_projection_events,
                quarantine_retention: config.retention_quarantine,
                interval: config.retention_interval,
                batch_size: config.retention_batch_size,
                database_max_bytes: config.database_max_bytes,
            },
            queue_capacity: config.collector_queue_capacity,
            maximum_active_windows: config.discovery_max_active_windows,
            discovery_rpc_requests_per_second: config.solana_discovery_rpc_requests_per_second,
            discovery_rpc_rate_limit_cooldown: config.solana_discovery_rpc_rate_limit_cooldown,
            prefilter_revision: 1,
            prefilter_values: config.prefilter_defaults(),
            collector_run_id: String::new(),
        }
    }
}

impl PipelineConfig {
    #[must_use]
    pub fn with_prefilter_defaults(&self, stored: StoredPrefilterDefaults) -> Self {
        let values = stored.values;
        let mut next = self.clone();
        next.collector.maximum_discovery_age = Duration::from_millis(values.max_event_age_ms);
        next.collector.observation_window_duration =
            Duration::from_millis(values.observation_window_ms);
        next.collector.rpc_max_in_flight = usize::try_from(values.rpc_max_in_flight)
            .expect("supported RPC concurrency fits usize");
        next.collector.request_timeout = Duration::from_millis(values.rpc_request_timeout_ms);
        next.maximum_active_windows = usize::try_from(values.max_active_windows)
            .expect("supported active-window limit fits usize");
        next.discovery_rpc_requests_per_second = values.rpc_requests_per_second;
        next.discovery_rpc_rate_limit_cooldown =
            Duration::from_millis(values.rpc_rate_limit_cooldown_ms);
        next.prefilter_revision = stored.revision;
        next.prefilter_values = values;
        next
    }

    #[must_use]
    pub fn discovery_rpc_gate(&self) -> DiscoveryRpcGate {
        DiscoveryRpcGate::new(
            self.collector.rpc_max_in_flight,
            self.discovery_rpc_requests_per_second,
            self.discovery_rpc_rate_limit_cooldown,
            self.collector
                .maximum_discovery_age
                .saturating_add(DISCOVERY_SIGNATURE_RETENTION_MARGIN),
        )
    }
}

pub struct SpawnedPipeline {
    pub cancellation: CancellationToken,
    pub status: watch::Receiver<StreamStatus>,
    pub task: JoinHandle<Result<(), PipelineError>>,
}

#[derive(Debug, Error)]
pub enum PipelineError {
    #[error(transparent)]
    Rpc(#[from] RpcError),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error(transparent)]
    Collector(#[from] CollectorError),
    #[error(transparent)]
    LogScope(#[from] LogScopeError),
    #[error(transparent)]
    Normalization(#[from] NormalizationError),
    #[error(transparent)]
    Maintenance(#[from] MaintenanceError),
    #[error("could not verify the configured Solana RPC network: {0}")]
    NetworkVerification(#[source] RpcError),
    #[error(
        "Solana RPC genesis hash does not match {network:?}: expected {expected}, received {actual}"
    )]
    NetworkGenesisMismatch {
        network: Network,
        expected: &'static str,
        actual: String,
    },
    #[error("pipeline task {0} exited unexpectedly")]
    UnexpectedTaskExit(&'static str),
    #[error("pipeline task failed to join: {0}")]
    Join(String),
    #[error("durable observation-window lifecycle became inconsistent: {0}")]
    WindowLifecycle(&'static str),
    #[error(transparent)]
    Qualification(#[from] QualificationWorkerError),
}

pub async fn spawn_pipeline(
    database: Database,
    events: LiveEventBus,
    config: PipelineConfig,
    discovery_rpc: DiscoveryRpcGate,
) -> Result<SpawnedPipeline, PipelineError> {
    let cancellation = CancellationToken::new();
    let (status_sender, status) = watch::channel(StreamStatus::Starting);
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        run_supervised_pipeline(
            database,
            events,
            config,
            discovery_rpc,
            status_sender,
            task_cancellation,
        )
        .await
    });

    Ok(SpawnedPipeline {
        cancellation,
        status,
        task,
    })
}

async fn run_supervised_pipeline(
    database: Database,
    events: LiveEventBus,
    config: PipelineConfig,
    discovery_rpc: DiscoveryRpcGate,
    status: watch::Sender<StreamStatus>,
    cancellation: CancellationToken,
) -> Result<(), PipelineError> {
    let mut restart_delay = PIPELINE_RESTART_INITIAL_DELAY;
    let mut first_attempt = true;

    loop {
        if cancellation.is_cancelled() {
            return Ok(());
        }
        status.send_replace(if first_attempt {
            StreamStatus::Starting
        } else {
            StreamStatus::Degraded
        });

        let attempt_cancellation = cancellation.child_token();
        let result = async {
            let http = Arc::new(SolanaHttpClient::with_timeout(
                &config.http_url,
                config.collector.request_timeout,
            )?);
            verify_rpc_network(&http, config.collector.network).await?;
            database.bind_network(config.collector.network).await?;
            run_maintenance_cycle(&database, &config.maintenance, &attempt_cancellation).await?;
            if attempt_cancellation.is_cancelled() {
                return Ok(());
            }
            let pubsub = SolanaPubsubClient::new(&config.ws_url)?;
            let markets = hydrate_market_registry(&database, &config).await?;
            run_pipeline(PipelineAttempt {
                database: database.clone(),
                events: events.clone(),
                http,
                pubsub,
                markets,
                config: config.clone(),
                discovery_rpc: discovery_rpc.clone(),
                status: status.clone(),
                cancellation: attempt_cancellation,
            })
            .await
        }
        .await;

        if cancellation.is_cancelled() {
            return Ok(());
        }
        let error = match result {
            Ok(()) => PipelineError::UnexpectedTaskExit("pipeline-attempt"),
            Err(error) => error,
        };
        if !pipeline_error_is_retryable(&error) {
            tracing::error!(%error, "discovery pipeline stopped on a non-retryable safety error");
            status.send_replace(StreamStatus::Error);
            return Err(error);
        }
        tracing::error!(
            %error,
            retry_delay_ms = restart_delay.as_millis(),
            "discovery pipeline attempt failed; restarting"
        );
        status.send_replace(StreamStatus::Degraded);
        first_attempt = false;

        tokio::select! {
            () = cancellation.cancelled() => return Ok(()),
            () = tokio::time::sleep(restart_delay) => {}
        }
        restart_delay = next_restart_delay(restart_delay);
    }
}

fn pipeline_error_is_retryable(error: &PipelineError) -> bool {
    !matches!(
        error,
        PipelineError::Maintenance(MaintenanceError::StorageLimitReached { .. })
            | PipelineError::NetworkGenesisMismatch { .. }
    )
}

async fn verify_rpc_network(
    http: &SolanaHttpClient,
    network: Network,
) -> Result<(), PipelineError> {
    let actual = http
        .genesis_hash()
        .await
        .map_err(PipelineError::NetworkVerification)?;
    let expected = match network {
        Network::SolanaMainnet => SOLANA_MAINNET_GENESIS_HASH,
        Network::SolanaDevnet => SOLANA_DEVNET_GENESIS_HASH,
    };
    if actual != expected {
        return Err(PipelineError::NetworkGenesisMismatch {
            network,
            expected,
            actual,
        });
    }
    Ok(())
}

fn next_restart_delay(current: Duration) -> Duration {
    current.saturating_mul(2).min(PIPELINE_RESTART_MAX_DELAY)
}

async fn hydrate_market_registry(
    database: &Database,
    config: &PipelineConfig,
) -> Result<MarketRegistry, PipelineError> {
    let registry = MarketRegistry::default();
    registry.register_all(
        database
            .load_discovery_markets(config.collector.network)
            .await?
            .into_iter()
            .filter(|market| market.venue == Venue::PumpBondingCurve),
    )?;
    for pool in database
        .load_pump_swap_pools(config.collector.network)
        .await?
    {
        let registered = registry.register_pump_swap_pool(
            pool.network,
            &pool.pool_address,
            &pool.base_mint,
            &pool.quote_mint,
        )?;
        if registered.is_none() {
            tracing::warn!(
                pool = %pool.pool_address,
                source_base_mint = %pool.base_mint,
                source_quote_mint = %pool.quote_mint,
                "ignored a persisted PumpSwap pool without exactly one supported quote asset"
            );
        }
    }
    Ok(registry)
}

struct PipelineAttempt {
    database: Database,
    events: LiveEventBus,
    http: Arc<SolanaHttpClient>,
    pubsub: SolanaPubsubClient,
    markets: MarketRegistry,
    config: PipelineConfig,
    discovery_rpc: DiscoveryRpcGate,
    status: watch::Sender<StreamStatus>,
    cancellation: CancellationToken,
}

async fn run_pipeline(attempt: PipelineAttempt) -> Result<(), PipelineError> {
    let PipelineAttempt {
        database,
        events,
        http,
        pubsub,
        markets,
        mut config,
        discovery_rpc,
        status,
        cancellation,
    } = attempt;
    config.collector_run_id = format!(
        "soldisco-collector-{}-{}",
        std::process::id(),
        unix_time_millis()
    );
    let (batch_sender, batch_receiver) = mpsc::channel(config.queue_capacity);
    let (connection_sender, mut connection_receiver) = mpsc::unbounded_channel();
    let (discovery_rpc_health_sender, mut discovery_rpc_health_receiver) =
        mpsc::unbounded_channel();
    let discovery_wake = Arc::new(Notify::new());
    let observation_windows = ObservationWindowRegistry::new(config.maximum_active_windows);
    let mut tasks = JoinSet::new();

    spawn_source(
        &mut tasks,
        PumpProgram::Pump,
        ProgramSourceContext {
            http: http.clone(),
            pubsub: pubsub.clone(),
            batches: batch_sender.clone(),
            connections: connection_sender.clone(),
            discovery_rpc_health: discovery_rpc_health_sender.clone(),
            windows: observation_windows.clone(),
            discovery_rpc: discovery_rpc.clone(),
            config: config.collector.clone(),
        },
        cancellation.child_token(),
    );
    spawn_source(
        &mut tasks,
        PumpProgram::PumpSwap,
        ProgramSourceContext {
            http,
            pubsub,
            batches: batch_sender,
            connections: connection_sender,
            discovery_rpc_health: discovery_rpc_health_sender,
            windows: observation_windows.clone(),
            discovery_rpc,
            config: config.collector.clone(),
        },
        cancellation.child_token(),
    );

    let processor_database = database.clone();
    let processor_events = events.clone();
    let processor_wake = discovery_wake.clone();
    let processor_cancellation = cancellation.child_token();
    let processor_config = config.clone();
    tasks.spawn(async move {
        (
            "collector-processor",
            run_batch_processor(
                processor_database,
                processor_events,
                markets,
                batch_receiver,
                processor_wake,
                processor_cancellation,
                processor_config,
            )
            .await,
        )
    });

    let qualification_database = database.clone();
    let qualification_events = events.clone();
    let qualification_windows = observation_windows.clone();
    let qualification_run_id = config.collector_run_id.clone();
    let qualification_cancellation = cancellation.child_token();
    tasks.spawn(async move {
        (
            "qualification-finalizer",
            run_qualification_finalizer(
                qualification_database,
                qualification_events,
                qualification_windows,
                qualification_run_id,
                qualification_cancellation,
            )
            .await
            .map_err(PipelineError::from),
        )
    });

    let maintenance_database = database.clone();
    let maintenance_cancellation = cancellation.child_token();
    let maintenance_config = config.maintenance.clone();
    tasks.spawn(async move {
        (
            "storage-maintenance",
            run_maintenance(
                maintenance_database,
                maintenance_config,
                maintenance_cancellation,
            )
            .await
            .map_err(PipelineError::from),
        )
    });

    let worker_cancellation = cancellation.child_token();
    tasks.spawn(async move {
        (
            "discovery-worker",
            run_discovery_worker(
                database,
                events,
                discovery_wake,
                worker_cancellation,
                DiscoveryWorkerConfig::for_process(),
            )
            .await
            .map_err(PipelineError::from),
        )
    });

    let mut ready_sources = HashSet::new();
    let mut sources_ready_once = HashSet::new();
    let mut connection_failed = false;
    let mut consecutive_discovery_rpc_failures = 0_u32;
    loop {
        tokio::select! {
            () = cancellation.cancelled() => {
                status.send_replace(StreamStatus::Stopping);
                observation_windows.cancel_all_confirmed(
                    soldisco_discovery_engine::WindowIncompleteReason::StreamStopped,
                );
                while tasks.join_next().await.is_some() {}
                return Ok(());
            }
            update = connection_receiver.recv() => {
                let Some(update) = update else {
                    return finish_with_error(
                        &status,
                        &cancellation,
                        &mut tasks,
                        PipelineError::UnexpectedTaskExit("source-status-channel"),
                    ).await;
                };
                match update.state {
                    SourceConnectionState::Ready => {
                        ready_sources.insert(update.source_program);
                        sources_ready_once.insert(update.source_program);
                    }
                    SourceConnectionState::Disconnected => {
                        ready_sources.remove(&update.source_program);
                        connection_failed = true;
                        observation_windows.cancel_source_confirmed(
                            update.source_program,
                            soldisco_discovery_engine::WindowIncompleteReason::SourceDisconnected,
                        );
                    }
                    SourceConnectionState::Connecting => {
                        ready_sources.remove(&update.source_program);
                        if sources_ready_once.contains(&update.source_program) {
                            connection_failed = true;
                        }
                    }
                }
                let all_sources_ready = ready_sources.contains(&SourceProgram::Pump)
                    && ready_sources.contains(&SourceProgram::PumpSwap);
                if all_sources_ready {
                    connection_failed = false;
                }
                status.send_replace(stream_status(
                    all_sources_ready,
                    connection_failed,
                    consecutive_discovery_rpc_failures,
                ));
            }
            update = discovery_rpc_health_receiver.recv() => {
                let Some(update) = update else {
                    return finish_with_error(
                        &status,
                        &cancellation,
                        &mut tasks,
                        PipelineError::UnexpectedTaskExit("discovery-rpc-health-channel"),
                    ).await;
                };
                consecutive_discovery_rpc_failures = match update {
                    DiscoveryRpcHealthUpdate::Successful => 0,
                    DiscoveryRpcHealthUpdate::Failed => {
                        consecutive_discovery_rpc_failures.saturating_add(1)
                    }
                    DiscoveryRpcHealthUpdate::RateLimited => {
                        DISCOVERY_RPC_FAILURES_BEFORE_DEGRADED
                    }
                };
                let all_sources_ready = ready_sources.contains(&SourceProgram::Pump)
                    && ready_sources.contains(&SourceProgram::PumpSwap);
                status.send_replace(stream_status(
                    all_sources_ready,
                    connection_failed,
                    consecutive_discovery_rpc_failures,
                ));
            }
            joined = tasks.join_next() => {
                let error = match joined {
                    Some(Ok((name, Ok(())))) => PipelineError::UnexpectedTaskExit(name),
                    Some(Ok((_, Err(error)))) => error,
                    Some(Err(error)) => PipelineError::Join(error.to_string()),
                    None => PipelineError::UnexpectedTaskExit("task-set"),
                };
                return finish_with_error(&status, &cancellation, &mut tasks, error).await;
            }
        }
    }
}

fn stream_status(
    all_sources_ready: bool,
    connection_failed: bool,
    consecutive_discovery_rpc_failures: u32,
) -> StreamStatus {
    let discovery_rpc_degraded =
        consecutive_discovery_rpc_failures >= DISCOVERY_RPC_FAILURES_BEFORE_DEGRADED;
    if all_sources_ready && !discovery_rpc_degraded {
        StreamStatus::Running
    } else if connection_failed || discovery_rpc_degraded {
        StreamStatus::Degraded
    } else {
        StreamStatus::Starting
    }
}

fn spawn_source(
    tasks: &mut JoinSet<(&'static str, Result<(), PipelineError>)>,
    program: PumpProgram,
    context: ProgramSourceContext,
    cancellation: CancellationToken,
) {
    let name = match program {
        PumpProgram::Pump => "pump-source",
        PumpProgram::PumpSwap => "pump-swap-source",
    };
    tasks.spawn(async move {
        (
            name,
            run_program_source(program, context, cancellation)
                .await
                .map_err(PipelineError::from),
        )
    });
}

async fn finish_with_error(
    status: &watch::Sender<StreamStatus>,
    cancellation: &CancellationToken,
    tasks: &mut JoinSet<(&'static str, Result<(), PipelineError>)>,
    error: PipelineError,
) -> Result<(), PipelineError> {
    status.send_replace(StreamStatus::Degraded);
    cancellation.cancel();
    while tasks.join_next().await.is_some() {}
    Err(error)
}

#[allow(clippy::too_many_arguments)]
async fn run_batch_processor(
    database: Database,
    events: LiveEventBus,
    markets: MarketRegistry,
    mut batches: mpsc::Receiver<ProgramLogBatch>,
    discovery_wake: Arc<Notify>,
    cancellation: CancellationToken,
    config: PipelineConfig,
) -> Result<(), PipelineError> {
    let mut recent_observations = VecDeque::<Instant>::new();
    let pending_activity_maximum_hold = config
        .collector
        .maximum_discovery_age
        .saturating_add(config.collector.request_timeout);
    let mut pending_activity =
        PendingActivityQueue::new(config.queue_capacity, pending_activity_maximum_hold);
    let mut current_flow = None;
    let mut flow_tick = tokio::time::interval(Duration::from_secs(1));
    let mut pending_tick = tokio::time::interval(Duration::from_millis(100));
    pending_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut storage_tick =
        tokio::time::interval(config.maintenance.interval.min(Duration::from_secs(5)));
    storage_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            () = cancellation.cancelled() => return Ok(()),
            _ = storage_tick.tick() => {
                ensure_storage_capacity(
                    &database,
                    config.maintenance.database_max_bytes,
                ).await?;
            }
            _ = flow_tick.tick() => {
                prune_flow(&mut recent_observations);
                let next_flow = Some(u64::try_from(recent_observations.len()).unwrap_or(u64::MAX));
                if next_flow != current_flow {
                    let changed = database.set_discovery_flow_per_minute(next_flow).await?;
                    current_flow = next_flow;
                    if changed {
                        events.publish_discovery_projection_changed();
                    }
                }
            }
            _ = pending_tick.tick() => {
                for batch in pending_activity.drain_resolved(unix_time_millis()) {
                    process_and_record_batch(
                        &database,
                        &markets,
                        &discovery_wake,
                        &config,
                        &mut recent_observations,
                        batch,
                    ).await?;
                }
            }
            batch = batches.recv() => {
                let Some(batch) = batch else {
                    return Err(PipelineError::UnexpectedTaskExit("collector-queue"));
                };
                match pending_activity.admit(batch, unix_time_millis()) {
                    PendingActivityAdmission::Ready(batch) => {
                        process_and_record_batch(
                            &database,
                            &markets,
                            &discovery_wake,
                            &config,
                            &mut recent_observations,
                            batch,
                        ).await?;
                    }
                    PendingActivityAdmission::Deferred => {}
                    PendingActivityAdmission::Dropped => {
                        tracing::warn!(
                            capacity = config.queue_capacity,
                            "dropped provisional activity because its bounded holding queue is full"
                        );
                    }
                }
                for batch in pending_activity.drain_resolved(unix_time_millis()) {
                    process_and_record_batch(
                        &database,
                        &markets,
                        &discovery_wake,
                        &config,
                        &mut recent_observations,
                        batch,
                    ).await?;
                }
            }
        }
    }
}

async fn process_and_record_batch(
    database: &Database,
    markets: &MarketRegistry,
    discovery_wake: &Notify,
    config: &PipelineConfig,
    recent_observations: &mut VecDeque<Instant>,
    mut batch: ProgramLogBatch,
) -> Result<(), PipelineError> {
    let outcome = process_batch(database, markets, &batch, config).await?;
    if !outcome.complete {
        for token in &batch.window_tokens {
            let _ =
                token.mark_incomplete(soldisco_discovery_engine::WindowIncompleteReason::DecodeGap);
        }
    }
    batch.mark_processing_succeeded();
    for _ in 0..outcome.inserted {
        recent_observations.push_back(Instant::now());
    }
    if outcome.inserted > 0 {
        discovery_wake.notify_one();
    }
    Ok(())
}

struct BatchProcessingOutcome {
    inserted: usize,
    complete: bool,
}

async fn process_batch(
    database: &Database,
    markets: &MarketRegistry,
    batch: &ProgramLogBatch,
    config: &PipelineConfig,
) -> Result<BatchProcessingOutcome, PipelineError> {
    let mut inserted = 0_usize;
    let mut complete = batch.transaction_error.is_none();
    let mut selected_events = 0_usize;

    if batch.transaction_error.is_none() {
        let mut decoded_events = Vec::new();
        for program in PUMP_PROGRAMS {
            let decoded_batch = decode_batch_events(batch, program)?;
            if !decoded_batch.quarantines.is_empty() {
                complete = false;
            }
            for quarantine in decoded_batch.quarantines {
                record_pipeline_quarantine(
                    database,
                    config.collector.network,
                    program.source_program(),
                    quarantine.coordinate,
                    quarantine.reason_code,
                    quarantine.reason_detail,
                    &quarantine.raw_evidence,
                )
                .await?;
            }
            decoded_events.extend(decoded_batch.events);
        }
        decoded_events.sort_by_key(|event| {
            let program_order = match event.decoded.program {
                PumpProgram::Pump => 0_u8,
                PumpProgram::PumpSwap => 1_u8,
            };
            (
                !is_discovery_event(&event.decoded.event),
                program_order,
                event.decoded.coordinate.instruction_index,
                event.decoded.coordinate.event_index,
            )
        });

        for event in decoded_events {
            let decoded = event.decoded;
            if !batch_selects_event(
                batch,
                &decoded.event,
                config.collector.network,
                config.collector.maximum_discovery_age,
            ) {
                continue;
            }
            selected_events = selected_events.saturating_add(1);
            let source_program = decoded.program.source_program();
            let coordinate = decoded.coordinate.clone();
            let retire_pump_mint = match &decoded.event {
                PumpEvent::Complete(event) => Some(event.mint.clone()),
                PumpEvent::CompletePumpAmmMigration(event) => Some(event.mint.clone()),
                _ => None,
            };
            let pump_swap_pool = match &decoded.event {
                PumpEvent::PumpSwapCreatePool(event) => Some(PumpSwapPool {
                    network: config.collector.network,
                    pool_address: event.pool.clone(),
                    base_mint: event.base_mint.clone(),
                    quote_mint: event.quote_mint.clone(),
                    observed_slot: decoded.coordinate.slot,
                    observed_signature: decoded.coordinate.signature.clone(),
                }),
                _ => None,
            };
            let normalized = match normalize_pump_event(
                decoded,
                config.collector.network,
                config.collector.commitment,
                batch.received_time_unix_ms,
                &event.raw_evidence,
                markets,
            ) {
                Ok(normalized) => normalized,
                Err(error @ NormalizationError::PumpMarketUnresolved(_)) => {
                    complete = false;
                    record_pipeline_quarantine(
                        database,
                        config.collector.network,
                        source_program,
                        coordinate,
                        "UNRESOLVED_PUMP_MARKET",
                        error.to_string(),
                        &event.raw_evidence,
                    )
                    .await?;
                    continue;
                }
                Err(error @ NormalizationError::PumpSwapMarketUnresolved(_)) => {
                    complete = false;
                    record_pipeline_quarantine(
                        database,
                        config.collector.network,
                        source_program,
                        coordinate,
                        "UNRESOLVED_PUMP_SWAP_MARKET",
                        error.to_string(),
                        &event.raw_evidence,
                    )
                    .await?;
                    continue;
                }
                Err(error @ NormalizationError::QuoteMintMismatch { .. }) => {
                    complete = false;
                    record_pipeline_quarantine(
                        database,
                        config.collector.network,
                        source_program,
                        coordinate,
                        "PUMP_QUOTE_MINT_MISMATCH",
                        error.to_string(),
                        &event.raw_evidence,
                    )
                    .await?;
                    continue;
                }
                Err(error @ NormalizationError::UnsupportedPumpSwapPair { .. }) => {
                    complete = false;
                    record_pipeline_quarantine(
                        database,
                        config.collector.network,
                        source_program,
                        coordinate,
                        "UNSUPPORTED_PUMP_SWAP_PAIR",
                        error.to_string(),
                        &event.raw_evidence,
                    )
                    .await?;
                    continue;
                }
                Err(error @ NormalizationError::ZeroTradeAmount) => {
                    complete = false;
                    record_pipeline_quarantine(
                        database,
                        config.collector.network,
                        source_program,
                        coordinate,
                        "ZERO_TRADE_AMOUNT",
                        error.to_string(),
                        &event.raw_evidence,
                    )
                    .await?;
                    continue;
                }
                Err(error) => return Err(error.into()),
            };

            let matching_window_token = soldisco_discovery_engine::ObservationTarget::for_market(
                &normalized.observation.market,
            )
            .and_then(|target| {
                batch
                    .window_tokens
                    .iter()
                    .find(|token| {
                        token.target() == &target && token.is_open_at(batch.received_time_unix_ms)
                    })
                    .cloned()
            });
            let observation_inserted = if let Some(market) = &normalized.discovered_market {
                let token =
                    matching_window_token
                        .as_ref()
                        .ok_or(PipelineError::WindowLifecycle(
                            "normalized discovery had no matching provisional token",
                        ))?;
                let qualification = database.load_qualification_defaults().await?;
                let (target_kind, target_address) = window_target_parts(token.target());
                let opened = database
                    .open_discovery_window(
                        &normalized.observation,
                        &NewDiscoveryWindow {
                            target_kind,
                            target_address,
                            opened_at_unix_ms: token.snapshot().opened_at_unix_ms,
                            closes_at_unix_ms: token.snapshot().closes_at_unix_ms,
                            collector_run_id: config.collector_run_id.clone(),
                            prefilter_revision: config.prefilter_revision,
                            prefilter_values: config.prefilter_values,
                            qualification,
                        },
                    )
                    .await?;
                if opened.replayed {
                    token.cancel_if_pending();
                    tracing::debug!(
                        window_id = opened.window_id,
                        mint = %market.mint,
                        "ignored a replayed discovery already handled by a durable window"
                    );
                    continue;
                }
                if !token.confirm() {
                    return Err(PipelineError::WindowLifecycle(
                        "durable discovery window could not confirm its provisional token",
                    ));
                }
                if !token.set_durable_window_id(opened.window_id) {
                    return Err(PipelineError::WindowLifecycle(
                        "confirmed token rejected its durable window id",
                    ));
                }
                tracing::debug!(
                    window_id = opened.window_id,
                    mint = %market.mint,
                    closes_at_unix_ms = token.snapshot().closes_at_unix_ms,
                    qualification_revision = qualification.revision,
                    "confirmed durable post-discovery observation window"
                );
                opened.observation_inserted
            } else if let Some(window_id) = matching_window_token
                .as_ref()
                .and_then(|token| token.durable_window_id())
            {
                database
                    .insert_window_observation(window_id, &normalized.observation)
                    .await?
            } else {
                database.insert_observation(&normalized.observation).await?
            };
            if observation_inserted {
                inserted = inserted.saturating_add(1);
            }
            if let Some(pool) = &pump_swap_pool {
                database.upsert_pump_swap_pool(pool).await?;
                let registered = markets.register_pump_swap_pool(
                    pool.network,
                    &pool.pool_address,
                    &pool.base_mint,
                    &pool.quote_mint,
                )?;
                if registered.as_ref() != normalized.discovered_market.as_ref() {
                    return Err(PipelineError::WindowLifecycle(
                        "PumpSwap source orientation disagreed with normalized discovery",
                    ));
                }
            }
            if let Some(market) = normalized
                .discovered_market
                .filter(|market| market.venue == Venue::PumpBondingCurve)
            {
                markets.register(market.clone())?;
            }
            if let Some(mint) = retire_pump_mint {
                markets.retire_pump_market(&mint)?;
            }
        }
    }

    if batch.purpose == BatchPurpose::Discovery {
        for token in &batch.window_tokens {
            token.cancel_if_pending();
        }
    }

    if !batch.window_tokens.is_empty() && selected_events == 0 {
        complete = false;
    }
    Ok(BatchProcessingOutcome { inserted, complete })
}

fn window_target_parts(
    target: &soldisco_discovery_engine::ObservationTarget,
) -> (WindowTargetKind, String) {
    match target {
        soldisco_discovery_engine::ObservationTarget::PumpMint(mint) => {
            (WindowTargetKind::PumpMint, mint.clone())
        }
        soldisco_discovery_engine::ObservationTarget::PumpSwapPool(pool) => {
            (WindowTargetKind::PumpSwapPool, pool.clone())
        }
    }
}

fn batch_selects_event(
    batch: &ProgramLogBatch,
    event: &PumpEvent,
    network: Network,
    maximum_discovery_age: Duration,
) -> bool {
    match batch.purpose {
        BatchPurpose::Discovery => {
            let selected_discovery = discovery_seed(event, network).is_some_and(|seed| {
                batch.window_tokens.iter().any(|token| {
                    token.target() == &seed.target && token.is_open_at(batch.received_time_unix_ms)
                })
            }) && discovery_event_is_fresh(
                event,
                batch.received_time_unix_ms,
                maximum_discovery_age,
            );
            selected_discovery
                || event_matches_confirmed_window(
                    event,
                    &batch.window_tokens,
                    batch.received_time_unix_ms,
                )
        }
        BatchPurpose::TrackedActivity => {
            event_matches_confirmed_window(event, &batch.window_tokens, batch.received_time_unix_ms)
        }
    }
}

async fn record_pipeline_quarantine(
    database: &Database,
    network: Network,
    source_program: SourceProgram,
    coordinate: soldisco_domain::ChainCoordinate,
    reason_code: &'static str,
    reason_detail: String,
    raw_evidence: &[u8],
) -> Result<(), PipelineError> {
    let signature = coordinate.signature.clone();
    let occurrences = database
        .record_intake_quarantine(&IntakeQuarantineRecord {
            key: ObservationKey {
                network,
                program: source_program,
                coordinate,
            },
            decoder_version: DECODER_VERSION.to_owned(),
            reason_code: reason_code.to_owned(),
            reason_detail,
            evidence_base64: encode_quarantine_evidence(raw_evidence),
        })
        .await?;
    tracing::warn!(
        source = ?source_program,
        %signature,
        reason_code,
        occurrences,
        "quarantined Pump source evidence"
    );
    Ok(())
}

#[derive(Clone)]
struct DecodedEvidence {
    decoded: DecodedPumpEvent,
    raw_evidence: Vec<u8>,
}

struct QuarantinedEvidence {
    coordinate: soldisco_domain::ChainCoordinate,
    reason_code: &'static str,
    reason_detail: String,
    raw_evidence: Vec<u8>,
}

struct DecodedBatch {
    events: Vec<DecodedEvidence>,
    quarantines: Vec<QuarantinedEvidence>,
}

fn decode_batch_events(
    batch: &ProgramLogBatch,
    program: PumpProgram,
) -> Result<DecodedBatch, PipelineError> {
    let program_id = program.program_id();
    let mut cpi_event_indexes = BTreeMap::<u16, u16>::new();
    let mut cpi_events = Vec::new();
    let mut quarantines = Vec::new();

    for instruction in batch.instructions.iter().filter(|instruction| {
        instruction.is_inner()
            && instruction.program_id == program_id
            && is_anchor_event_cpi(&instruction.data)
    }) {
        let event_index = cpi_event_indexes
            .entry(instruction.outer_instruction_index)
            .or_default();
        let coordinate = soldisco_domain::ChainCoordinate {
            slot: batch.slot,
            transaction_index: batch.transaction_index,
            signature: batch.signature.clone(),
            instruction_index: instruction.outer_instruction_index,
            event_index: *event_index,
        };
        *event_index = event_index.checked_add(1).ok_or(PipelineError::LogScope(
            LogScopeError::TooManyEvents(u16::MAX),
        ))?;
        match decode_cpi_event(program_id, coordinate.clone(), &instruction.data) {
            Ok(event) => cpi_events.push(DecodedEvidence {
                decoded: event,
                raw_evidence: instruction.data.clone(),
            }),
            Err(DecodeError::UnknownDiscriminator {
                program,
                discriminator,
            }) if is_pinned_event_discriminator(program, discriminator) => {}
            Err(source @ DecodeError::UnknownDiscriminator { .. }) => {
                quarantines.push(QuarantinedEvidence {
                    coordinate,
                    reason_code: "UNKNOWN_PUMP_EVENT_DISCRIMINATOR",
                    reason_detail: source.to_string(),
                    raw_evidence: instruction.data.clone(),
                });
            }
            Err(source) => {
                quarantines.push(QuarantinedEvidence {
                    coordinate,
                    reason_code: "MALFORMED_PUMP_EVENT",
                    reason_detail: source.to_string(),
                    raw_evidence: instruction.data.clone(),
                });
            }
        }
    }

    let scoped_logs = match scope_program_data_logs(
        program_id,
        batch.slot,
        batch.transaction_index,
        &batch.signature,
        &batch.log_messages,
    ) {
        Ok(records) => records,
        Err(error) => {
            quarantines.push(QuarantinedEvidence {
                coordinate: soldisco_domain::ChainCoordinate {
                    slot: batch.slot,
                    transaction_index: batch.transaction_index,
                    signature: batch.signature.clone(),
                    instruction_index: 0,
                    event_index: 0,
                },
                reason_code: "MALFORMED_LOG_SCOPE",
                reason_detail: error.to_string(),
                raw_evidence: batch.log_messages.join("\n").into_bytes(),
            });
            return Ok(DecodedBatch {
                events: Vec::new(),
                quarantines,
            });
        }
    };

    // Program-data logs are the canonical event stream because they are
    // present in both PubSub notifications and getTransaction responses. CPI
    // instruction bytes are useful corroboration, but must never shift event
    // indexes or replace the raw evidence: doing so would give one chain event
    // different durable identities depending on which collection path won.
    let mut decoded = Vec::new();
    for record in scoped_logs {
        let coordinate = record.coordinate;
        let raw_evidence = match decode_program_data_bytes(&record.log) {
            Ok(evidence) => evidence,
            Err(source) => {
                quarantines.push(QuarantinedEvidence {
                    coordinate,
                    reason_code: "MALFORMED_PROGRAM_DATA",
                    reason_detail: source.to_string(),
                    raw_evidence: record.log.into_bytes(),
                });
                continue;
            }
        };

        match decode_anchor_event(program_id, coordinate.clone(), &raw_evidence) {
            Ok(event) => decoded.push(DecodedEvidence {
                decoded: event,
                raw_evidence,
            }),
            Err(DecodeError::UnknownDiscriminator {
                program,
                discriminator,
            }) if is_pinned_event_discriminator(program, discriminator) => {}
            Err(source @ DecodeError::UnknownDiscriminator { .. }) => {
                quarantines.push(QuarantinedEvidence {
                    coordinate,
                    reason_code: "UNKNOWN_PUMP_EVENT_DISCRIMINATOR",
                    reason_detail: source.to_string(),
                    raw_evidence,
                });
            }
            Err(source) => {
                quarantines.push(QuarantinedEvidence {
                    coordinate,
                    reason_code: "MALFORMED_PUMP_EVENT",
                    reason_detail: source.to_string(),
                    raw_evidence,
                });
            }
        }
    }

    let mut matched_direct_events = vec![false; decoded.len()];
    for cpi in cpi_events {
        if let Some((index, _)) = decoded.iter().enumerate().find(|(index, direct)| {
            !matched_direct_events[*index]
                && direct.decoded.coordinate.instruction_index
                    == cpi.decoded.coordinate.instruction_index
                && direct.decoded.event == cpi.decoded.event
        }) {
            matched_direct_events[index] = true;
        } else {
            quarantines.push(QuarantinedEvidence {
                coordinate: cpi.decoded.coordinate,
                reason_code: "CPI_EVENT_WITHOUT_CANONICAL_LOG",
                reason_detail: "decoded Anchor event CPI had no matching direct program-data event"
                    .to_owned(),
                raw_evidence: cpi.raw_evidence,
            });
        }
    }

    Ok(DecodedBatch {
        events: decoded,
        quarantines,
    })
}

fn encode_quarantine_evidence(raw_evidence: &[u8]) -> String {
    let evidence = if raw_evidence.is_empty() {
        b"<empty-evidence>".as_slice()
    } else {
        &raw_evidence[..raw_evidence.len().min(MAX_QUARANTINE_RAW_BYTES)]
    };
    STANDARD.encode(evidence)
}

fn prune_flow(observations: &mut VecDeque<Instant>) {
    let cutoff = Instant::now()
        .checked_sub(Duration::from_secs(60))
        .unwrap_or_else(Instant::now);
    while observations
        .front()
        .is_some_and(|observed| *observed < cutoff)
    {
        observations.pop_front();
    }
}

fn unix_time_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, time::Duration};

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use soldisco_api_contracts::PrefilterDefaults;
    use soldisco_discovery_engine::ObservationWindowRegistry;
    use soldisco_domain::{ChainCoordinate, Commitment, Network};
    use soldisco_persistence::StoredPrefilterDefaults;
    use soldisco_solana_rpc::TransactionInstructionRecord;
    use soldisco_source_pump::{
        ANCHOR_EVENT_CPI_DISCRIMINATOR, COMPLETE_EVENT_DISCRIMINATOR, CREATE_EVENT_DISCRIMINATOR,
        PUMP_PROGRAM_ID, PumpEvent, PumpProgram, decode_anchor_event,
    };

    use super::{
        BatchPurpose, DISCOVERY_RPC_FAILURES_BEFORE_DEGRADED, PIPELINE_RESTART_MAX_DELAY,
        PipelineConfig, PipelineError, ProgramLogBatch, SOLANA_DEVNET_GENESIS_HASH,
        SOLANA_MAINNET_GENESIS_HASH, batch_selects_event, decode_batch_events, next_restart_delay,
        pipeline_error_is_retryable, prune_flow, stream_status,
    };
    use crate::jobs::{
        collector::CollectorRuntimeConfig,
        maintenance::{MaintenanceError, MaintenanceRuntimeConfig},
    };

    fn complete_event_bytes(marker: u8) -> Vec<u8> {
        let mut bytes = COMPLETE_EVENT_DISCRIMINATOR.to_vec();
        bytes.extend([marker; 32]);
        bytes.extend([marker.saturating_add(1); 32]);
        bytes.extend([marker.saturating_add(2); 32]);
        bytes.extend(1_720_000_000_i64.to_le_bytes());
        bytes.extend([marker.saturating_add(3); 32]);
        bytes
    }

    fn push_string(bytes: &mut Vec<u8>, value: &str) {
        bytes.extend(
            u32::try_from(value.len())
                .expect("fixture string fits in u32")
                .to_le_bytes(),
        );
        bytes.extend(value.as_bytes());
    }

    fn create_event_bytes(timestamp: i64) -> Vec<u8> {
        let mut bytes = CREATE_EVENT_DISCRIMINATOR.to_vec();
        push_string(&mut bytes, "Fixture Coin");
        push_string(&mut bytes, "FIX");
        push_string(&mut bytes, "https://example.invalid/fixture.json");
        for marker in [2_u8, 3, 1, 5] {
            bytes.extend([marker; 32]);
        }
        bytes.extend(timestamp.to_le_bytes());
        for value in 1_u64..=4 {
            bytes.extend((value * 1_000).to_le_bytes());
        }
        bytes.extend([6_u8; 32]);
        bytes.push(0);
        bytes.push(0);
        bytes.extend([4_u8; 32]);
        bytes.extend(5_000_u64.to_le_bytes());
        bytes
    }

    fn coordinate(event_index: u16) -> ChainCoordinate {
        ChainCoordinate {
            slot: 42,
            transaction_index: Some(3),
            signature: "signature".to_owned(),
            instruction_index: 0,
            event_index,
        }
    }

    fn batch(
        instructions: Vec<TransactionInstructionRecord>,
        direct_event: Vec<u8>,
    ) -> ProgramLogBatch {
        batch_with_direct_events(instructions, vec![direct_event])
    }

    fn batch_with_direct_events(
        instructions: Vec<TransactionInstructionRecord>,
        direct_events: Vec<Vec<u8>>,
    ) -> ProgramLogBatch {
        let mut log_messages = vec![format!("Program {PUMP_PROGRAM_ID} invoke [1]")];
        log_messages.extend(
            direct_events
                .into_iter()
                .map(|event| format!("Program data: {}", STANDARD.encode(event))),
        );
        log_messages.push(format!("Program {PUMP_PROGRAM_ID} success"));
        ProgramLogBatch {
            purpose: BatchPurpose::TrackedActivity,
            slot: 42,
            transaction_index: Some(3),
            signature: "signature".to_owned(),
            received_time_unix_ms: 1_720_000_000_000,
            instructions,
            log_messages,
            transaction_error: None,
            window_tokens: Vec::new(),
            processing_succeeded: false,
        }
    }

    #[test]
    fn flow_window_starts_empty() {
        let mut observations = VecDeque::new();
        prune_flow(&mut observations);

        assert!(observations.is_empty());
    }

    #[test]
    fn pipeline_restart_backoff_is_capped() {
        assert_eq!(
            next_restart_delay(Duration::from_secs(20)),
            PIPELINE_RESTART_MAX_DELAY
        );
        assert_eq!(
            next_restart_delay(PIPELINE_RESTART_MAX_DELAY),
            PIPELINE_RESTART_MAX_DELAY
        );
    }

    #[test]
    fn persisted_prefilter_defaults_overlay_only_collector_admission_fields() {
        let base = PipelineConfig {
            http_url: "https://example.invalid".to_owned(),
            ws_url: "wss://example.invalid".to_owned(),
            collector: CollectorRuntimeConfig {
                network: Network::SolanaMainnet,
                commitment: Commitment::Confirmed,
                reconnect_delay: Duration::from_secs(1),
                request_timeout: Duration::from_secs(5),
                rpc_max_in_flight: 4,
                notification_processing_capacity: 2_048,
                subscription_idle_timeout: Duration::from_secs(30),
                maximum_discovery_age: Duration::from_secs(15),
                observation_window_duration: Duration::from_secs(60),
            },
            maintenance: MaintenanceRuntimeConfig {
                terminal_history_retention: Duration::from_secs(60),
                projection_event_retention: Duration::from_secs(60),
                quarantine_retention: Duration::from_secs(60),
                interval: Duration::from_secs(60),
                batch_size: 100,
                database_max_bytes: 1_000_000,
            },
            queue_capacity: 2_048,
            maximum_active_windows: 128,
            discovery_rpc_requests_per_second: 1,
            discovery_rpc_rate_limit_cooldown: Duration::from_secs(5),
            prefilter_revision: 1,
            prefilter_values: PrefilterDefaults {
                max_event_age_ms: 15_000,
                observation_window_ms: 60_000,
                max_active_windows: 128,
                rpc_requests_per_second: 1,
                rpc_max_in_flight: 4,
                rpc_request_timeout_ms: 5_000,
                rpc_rate_limit_cooldown_ms: 5_000,
            },
            collector_run_id: String::new(),
        };
        let next = base.with_prefilter_defaults(StoredPrefilterDefaults {
            revision: 7,
            values: PrefilterDefaults {
                max_event_age_ms: 20_000,
                observation_window_ms: 90_000,
                max_active_windows: 512,
                rpc_requests_per_second: 8,
                rpc_max_in_flight: 16,
                rpc_request_timeout_ms: 4_000,
                rpc_rate_limit_cooldown_ms: 9_000,
            },
        });

        assert_eq!(
            next.collector.maximum_discovery_age,
            Duration::from_secs(20)
        );
        assert_eq!(
            next.collector.observation_window_duration,
            Duration::from_secs(90)
        );
        assert_eq!(next.maximum_active_windows, 512);
        assert_eq!(next.collector.rpc_max_in_flight, 16);
        assert_eq!(next.collector.request_timeout, Duration::from_secs(4));
        assert_eq!(next.discovery_rpc_requests_per_second, 8);
        assert_eq!(
            next.discovery_rpc_rate_limit_cooldown,
            Duration::from_secs(9)
        );
        assert_eq!(next.queue_capacity, base.queue_capacity);
        assert_eq!(next.prefilter_revision, 7);
        assert_eq!(
            next.collector.notification_processing_capacity,
            base.collector.notification_processing_capacity
        );
    }

    #[test]
    fn discovery_rpc_failures_degrade_and_success_recovers_stream_status() {
        assert_eq!(
            stream_status(true, false, 0),
            soldisco_api_contracts::StreamStatus::Running
        );
        assert_eq!(
            stream_status(true, false, DISCOVERY_RPC_FAILURES_BEFORE_DEGRADED),
            soldisco_api_contracts::StreamStatus::Degraded
        );
        assert_eq!(
            stream_status(false, false, 0),
            soldisco_api_contracts::StreamStatus::Starting
        );
    }

    #[test]
    fn discovery_confirmation_admits_same_transaction_activity() {
        let received_at = 1_720_000_001_000_i64;
        let create = decode_anchor_event(
            PUMP_PROGRAM_ID,
            coordinate(0),
            &create_event_bytes(1_720_000_000),
        )
        .expect("create fixture");
        let complete =
            decode_anchor_event(PUMP_PROGRAM_ID, coordinate(1), &complete_event_bytes(1))
                .expect("matching lifecycle fixture");
        let seed =
            super::discovery_seed(&create.event, Network::SolanaMainnet).expect("discovery target");
        let windows = ObservationWindowRegistry::new(4);
        let provision =
            windows.provision_target(seed.target, seed.mint, received_at, Duration::from_secs(5));
        let batch = ProgramLogBatch {
            purpose: BatchPurpose::Discovery,
            slot: 42,
            transaction_index: Some(3),
            signature: "signature".to_owned(),
            received_time_unix_ms: received_at,
            instructions: Vec::new(),
            log_messages: Vec::new(),
            transaction_error: None,
            window_tokens: vec![provision.token.clone()],
            processing_succeeded: false,
        };

        assert!(batch_selects_event(
            &batch,
            &create.event,
            Network::SolanaMainnet,
            Duration::from_secs(5)
        ));
        assert!(!batch_selects_event(
            &batch,
            &complete.event,
            Network::SolanaMainnet,
            Duration::from_secs(5)
        ));
        assert!(provision.token.confirm());
        assert!(batch_selects_event(
            &batch,
            &complete.event,
            Network::SolanaMainnet,
            Duration::from_secs(5)
        ));
    }

    #[test]
    fn storage_limit_is_latched_instead_of_auto_restarted() {
        let error = PipelineError::Maintenance(MaintenanceError::StorageLimitReached {
            current_bytes: 10,
            maximum_bytes: 10,
        });

        assert!(!pipeline_error_is_retryable(&error));
    }

    #[test]
    fn official_program_constants_are_distinct() {
        assert_ne!(
            soldisco_source_pump::PUMP_PROGRAM_ID,
            soldisco_source_pump::PUMP_SWAP_PROGRAM_ID
        );
    }

    #[test]
    fn configured_networks_have_distinct_pinned_genesis_hashes() {
        assert_ne!(SOLANA_MAINNET_GENESIS_HASH, SOLANA_DEVNET_GENESIS_HASH);
        assert_eq!(SOLANA_MAINNET_GENESIS_HASH.len(), 44);
        assert_eq!(SOLANA_DEVNET_GENESIS_HASH.len(), 44);
    }

    #[test]
    fn cpi_copy_keeps_the_direct_log_as_canonical_evidence() {
        let mut cpi_data = ANCHOR_EVENT_CPI_DISCRIMINATOR.to_vec();
        let direct_evidence = complete_event_bytes(1);
        cpi_data.extend(&direct_evidence);
        let decoded = decode_batch_events(
            &batch(
                vec![TransactionInstructionRecord {
                    outer_instruction_index: 0,
                    inner_instruction_index: Some(2),
                    stack_height: Some(2),
                    program_id: PUMP_PROGRAM_ID.to_owned(),
                    data: cpi_data,
                }],
                direct_evidence.clone(),
            ),
            PumpProgram::Pump,
        )
        .expect("current CPI event should decode");
        let events = decoded.events;

        assert_eq!(events.len(), 1);
        assert!(decoded.quarantines.is_empty());
        assert!(matches!(events[0].decoded.event, PumpEvent::Complete(_)));
        assert_eq!(events[0].decoded.coordinate.event_index, 0);
        assert_eq!(events[0].raw_evidence, direct_evidence);
    }

    #[test]
    fn direct_program_data_is_used_when_instruction_has_no_cpi_event() {
        let evidence = complete_event_bytes(1);
        let decoded = decode_batch_events(&batch(Vec::new(), evidence.clone()), PumpProgram::Pump)
            .expect("current direct program-data event should decode");
        let events = decoded.events;

        assert_eq!(events.len(), 1);
        assert!(decoded.quarantines.is_empty());
        assert!(matches!(events[0].decoded.event, PumpEvent::Complete(_)));
        assert_eq!(events[0].decoded.coordinate.event_index, 0);
        assert_eq!(events[0].raw_evidence, evidence);
    }

    #[test]
    fn a_distinct_direct_event_survives_alongside_a_cpi_event() {
        let mut cpi_data = ANCHOR_EVENT_CPI_DISCRIMINATOR.to_vec();
        cpi_data.extend(complete_event_bytes(1));
        let decoded = decode_batch_events(
            &batch_with_direct_events(
                vec![TransactionInstructionRecord {
                    outer_instruction_index: 0,
                    inner_instruction_index: Some(2),
                    stack_height: Some(2),
                    program_id: PUMP_PROGRAM_ID.to_owned(),
                    data: cpi_data,
                }],
                vec![complete_event_bytes(1), complete_event_bytes(10)],
            ),
            PumpProgram::Pump,
        )
        .expect("mixed direct and CPI events should decode");
        let events = decoded.events;

        assert_eq!(events.len(), 2);
        assert!(decoded.quarantines.is_empty());
        assert_eq!(events[0].decoded.coordinate.event_index, 0);
        assert_eq!(events[1].decoded.coordinate.event_index, 1);
        assert_ne!(events[0].decoded.event, events[1].decoded.event);
    }

    #[test]
    fn event_identity_and_evidence_do_not_depend_on_http_cpi_availability() {
        let direct_events = vec![complete_event_bytes(1), complete_event_bytes(10)];
        let mut cpi_data = ANCHOR_EVENT_CPI_DISCRIMINATOR.to_vec();
        cpi_data.extend(&direct_events[0]);
        let with_http_cpi = decode_batch_events(
            &batch_with_direct_events(
                vec![TransactionInstructionRecord {
                    outer_instruction_index: 0,
                    inner_instruction_index: Some(2),
                    stack_height: Some(2),
                    program_id: PUMP_PROGRAM_ID.to_owned(),
                    data: cpi_data,
                }],
                direct_events.clone(),
            ),
            PumpProgram::Pump,
        )
        .expect("HTTP evidence path should decode");
        let pubsub_only = decode_batch_events(
            &batch_with_direct_events(Vec::new(), direct_events),
            PumpProgram::Pump,
        )
        .expect("PubSub-only evidence path should decode");

        assert!(with_http_cpi.quarantines.is_empty());
        assert!(pubsub_only.quarantines.is_empty());
        assert_eq!(with_http_cpi.events.len(), pubsub_only.events.len());
        for (with_cpi, without_cpi) in with_http_cpi.events.iter().zip(&pubsub_only.events) {
            assert_eq!(with_cpi.decoded, without_cpi.decoded);
            assert_eq!(with_cpi.raw_evidence, without_cpi.raw_evidence);
        }
    }

    #[test]
    fn cpi_without_a_canonical_direct_log_fails_the_batch_closed() {
        let mut cpi_data = ANCHOR_EVENT_CPI_DISCRIMINATOR.to_vec();
        cpi_data.extend(complete_event_bytes(1));
        let decoded = decode_batch_events(
            &batch(
                vec![TransactionInstructionRecord {
                    outer_instruction_index: 0,
                    inner_instruction_index: Some(2),
                    stack_height: Some(2),
                    program_id: PUMP_PROGRAM_ID.to_owned(),
                    data: cpi_data,
                }],
                complete_event_bytes(10),
            ),
            PumpProgram::Pump,
        )
        .expect("unpaired CPI should be isolated");

        assert_eq!(decoded.events.len(), 1);
        assert_eq!(decoded.events[0].decoded.coordinate.event_index, 0);
        assert_eq!(decoded.quarantines.len(), 1);
        assert_eq!(
            decoded.quarantines[0].reason_code,
            "CPI_EVENT_WITHOUT_CANONICAL_LOG"
        );
    }

    #[test]
    fn malformed_known_event_is_quarantined_without_poisoning_the_batch() {
        let decoded = decode_batch_events(
            &batch(Vec::new(), COMPLETE_EVENT_DISCRIMINATOR.to_vec()),
            PumpProgram::Pump,
        )
        .expect("malformed evidence should be isolated");

        assert!(decoded.events.is_empty());
        assert_eq!(decoded.quarantines.len(), 1);
        assert_eq!(decoded.quarantines[0].reason_code, "MALFORMED_PUMP_EVENT");
    }

    #[test]
    fn pinned_ignored_event_is_distinct_from_a_future_discriminator() {
        let known_ignored = decode_batch_events(
            &batch(Vec::new(), vec![64, 69, 192, 104, 29, 30, 25, 107]),
            PumpProgram::Pump,
        )
        .expect("known pinned event should be classified");
        assert!(known_ignored.events.is_empty());
        assert!(known_ignored.quarantines.is_empty());

        let future = decode_batch_events(&batch(Vec::new(), vec![255; 8]), PumpProgram::Pump)
            .expect("future event should be isolated");
        assert!(future.events.is_empty());
        assert_eq!(future.quarantines.len(), 1);
        assert_eq!(
            future.quarantines[0].reason_code,
            "UNKNOWN_PUMP_EVENT_DISCRIMINATOR"
        );
    }
}
