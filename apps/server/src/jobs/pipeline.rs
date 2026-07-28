use std::{
    collections::{BTreeMap, HashSet, VecDeque},
    sync::Arc,
    time::{Duration, Instant},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use soldisco_api_contracts::StreamStatus;
use soldisco_domain::{MarketIdentity, Network, ObservationKey, SourceProgram, Venue};
use soldisco_persistence::{
    Database, IntakeQuarantineRecord, MAX_QUARANTINE_EVIDENCE_BASE64_BYTES, PersistenceError,
    PumpSwapPool, RecoveryCheckpoint,
};
use soldisco_solana_rpc::{RpcError, SolanaHttpClient, SolanaPubsubClient};
use soldisco_source_pump::{
    DECODER_VERSION, DecodeError, DecodedPumpEvent, PumpEvent, PumpProgram, decode_anchor_event,
    decode_cpi_event, decode_program_data_bytes, is_anchor_event_cpi,
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
            CollectorError, CollectorRuntimeConfig, LogScopeError, ProgramLogBatch,
            ProgramSourceContext, SourceConnectionState, run_program_source,
            scope_program_data_logs,
        },
        discovery::{DiscoveryWorkerConfig, run_discovery_worker},
        maintenance::{
            MaintenanceError, MaintenanceRuntimeConfig, ensure_storage_capacity, run_maintenance,
            run_maintenance_cycle,
        },
        normalization::{MarketRegistry, NormalizationError, normalize_pump_event},
    },
    state::LiveEventBus,
};

const CHECKPOINT_KIND: &str = "PROGRAM_LOGS";
const PIPELINE_RESTART_INITIAL_DELAY: Duration = Duration::from_secs(1);
const PIPELINE_RESTART_MAX_DELAY: Duration = Duration::from_secs(30);
const MAX_QUARANTINE_RAW_BYTES: usize = (MAX_QUARANTINE_EVIDENCE_BASE64_BYTES / 4) * 3;
const SOLANA_MAINNET_GENESIS_HASH: &str = "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d";
const SOLANA_DEVNET_GENESIS_HASH: &str = "EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG";

#[derive(Clone)]
pub struct PipelineConfig {
    pub http_url: String,
    pub ws_url: String,
    pub collector: CollectorRuntimeConfig,
    pub maintenance: MaintenanceRuntimeConfig,
    pub queue_capacity: usize,
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
                live_fetch_max_attempts: config.solana_live_fetch_max_attempts,
                subscription_idle_timeout: config.solana_subscription_idle_timeout,
                recovery_page_size: config.recovery_page_size,
                recovery_max_records: config.recovery_max_records,
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
        }
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
}

pub async fn spawn_pipeline(
    database: Database,
    events: LiveEventBus,
    config: PipelineConfig,
) -> Result<SpawnedPipeline, PipelineError> {
    let cancellation = CancellationToken::new();
    let (status_sender, status) = watch::channel(StreamStatus::Starting);
    let task_cancellation = cancellation.clone();
    let task = tokio::spawn(async move {
        run_supervised_pipeline(database, events, config, status_sender, task_cancellation).await
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
            .filter(|market| matches!(market.venue, Venue::PumpBondingCurve | Venue::PumpSwap)),
    )?;
    registry.register_all(
        database
            .load_pump_swap_pools(config.collector.network)
            .await?
            .into_iter()
            .map(|pool| MarketIdentity {
                network: pool.network,
                mint: pool.base_mint,
                venue: Venue::PumpSwap,
                market_address: pool.pool_address,
                quote_mint: Some(pool.quote_mint),
            }),
    )?;
    Ok(registry)
}

struct PipelineAttempt {
    database: Database,
    events: LiveEventBus,
    http: Arc<SolanaHttpClient>,
    pubsub: SolanaPubsubClient,
    markets: MarketRegistry,
    config: PipelineConfig,
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
        config,
        status,
        cancellation,
    } = attempt;
    let (batch_sender, batch_receiver) = mpsc::channel(config.queue_capacity);
    let (connection_sender, mut connection_receiver) = mpsc::unbounded_channel();
    let discovery_wake = Arc::new(Notify::new());
    let mut tasks = JoinSet::new();

    spawn_source(
        &mut tasks,
        PumpProgram::Pump,
        ProgramSourceContext {
            database: database.clone(),
            http: http.clone(),
            pubsub: pubsub.clone(),
            batches: batch_sender.clone(),
            connections: connection_sender.clone(),
            config: config.collector.clone(),
        },
        cancellation.child_token(),
    );
    spawn_source(
        &mut tasks,
        PumpProgram::PumpSwap,
        ProgramSourceContext {
            database: database.clone(),
            http,
            pubsub,
            batches: batch_sender,
            connections: connection_sender,
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
    let mut gapped_sources = HashSet::new();
    let mut sources_ready_once = HashSet::new();
    let mut connection_failed = false;
    loop {
        tokio::select! {
            () = cancellation.cancelled() => {
                status.send_replace(StreamStatus::Stopping);
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
                        gapped_sources.remove(&update.source_program);
                        sources_ready_once.insert(update.source_program);
                    }
                    SourceConnectionState::ReadyWithGap => {
                        ready_sources.insert(update.source_program);
                        gapped_sources.insert(update.source_program);
                        sources_ready_once.insert(update.source_program);
                    }
                    SourceConnectionState::Disconnected => {
                        ready_sources.remove(&update.source_program);
                        connection_failed = true;
                    }
                    SourceConnectionState::Connecting | SourceConnectionState::Recovering => {
                        ready_sources.remove(&update.source_program);
                        if sources_ready_once.contains(&update.source_program) {
                            connection_failed = true;
                        }
                    }
                }
                status.send_replace(if ready_sources.contains(&SourceProgram::Pump)
                    && ready_sources.contains(&SourceProgram::PumpSwap)
                {
                    connection_failed = false;
                    if gapped_sources.is_empty() {
                        StreamStatus::Running
                    } else {
                        StreamStatus::Degraded
                    }
                } else if connection_failed {
                    StreamStatus::Degraded
                } else {
                    StreamStatus::Starting
                });
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
    let mut current_flow = None;
    let mut flow_tick = tokio::time::interval(Duration::from_secs(1));
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
            batch = batches.recv() => {
                let Some(batch) = batch else {
                    return Err(PipelineError::UnexpectedTaskExit("collector-queue"));
                };
                let inserted = process_batch(&database, &markets, &batch, &config).await?;
                for _ in 0..inserted {
                    recent_observations.push_back(Instant::now());
                }
                if inserted > 0 {
                    discovery_wake.notify_one();
                }
            }
        }
    }
}

async fn process_batch(
    database: &Database,
    markets: &MarketRegistry,
    batch: &ProgramLogBatch,
    config: &PipelineConfig,
) -> Result<usize, PipelineError> {
    let source_program = batch.program.source_program();
    let mut inserted = 0_usize;

    if batch.transaction_error.is_none() {
        let decoded_batch = decode_batch_events(batch)?;
        for quarantine in decoded_batch.quarantines {
            record_pipeline_quarantine(
                database,
                config.collector.network,
                source_program,
                quarantine.coordinate,
                quarantine.reason_code,
                quarantine.reason_detail,
                &quarantine.raw_evidence,
            )
            .await?;
        }

        for event in decoded_batch.events {
            let decoded = event.decoded;
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
                Err(error) => return Err(error.into()),
            };

            if database.insert_observation(&normalized.observation).await? {
                inserted = inserted.saturating_add(1);
            }
            if let Some(pool) = pump_swap_pool {
                database.upsert_pump_swap_pool(&pool).await?;
            }
            if let Some(market) = normalized.discovered_market {
                markets.register(market)?;
            }
            if let Some(mint) = retire_pump_mint {
                markets.retire_pump_market(&mint)?;
            }
        }
    }

    database
        .save_recovery_checkpoint(&RecoveryCheckpoint {
            network: config.collector.network,
            source_program,
            checkpoint_kind: CHECKPOINT_KIND.to_owned(),
            last_slot: batch.slot,
            last_transaction_index: batch.transaction_index,
            last_signature: batch.signature.clone(),
        })
        .await?;
    Ok(inserted)
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
        "quarantined Pump source evidence before advancing its checkpoint"
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

fn decode_batch_events(batch: &ProgramLogBatch) -> Result<DecodedBatch, PipelineError> {
    let program_id = batch.program.program_id();
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
            Err(DecodeError::UnknownDiscriminator { .. }) => {}
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

    let mut matched_cpi_events = vec![false; cpi_events.len()];
    let mut decoded = cpi_events.clone();
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
                events: decoded,
                quarantines,
            });
        }
    };

    for record in scoped_logs {
        let mut coordinate = record.coordinate;
        if let Some(cpi_count) = cpi_event_indexes.get(&coordinate.instruction_index) {
            coordinate.event_index =
                cpi_count
                    .checked_add(coordinate.event_index)
                    .ok_or(PipelineError::LogScope(LogScopeError::TooManyEvents(
                        u16::MAX,
                    )))?;
        }
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
            Ok(event) => {
                if let Some((index, _)) = cpi_events.iter().enumerate().find(|(index, cpi)| {
                    !matched_cpi_events[*index]
                        && cpi.decoded.coordinate.instruction_index
                            == event.coordinate.instruction_index
                        && cpi.decoded.event == event.event
                }) {
                    matched_cpi_events[index] = true;
                    continue;
                }

                decoded.push(DecodedEvidence {
                    decoded: event,
                    raw_evidence,
                });
            }
            Err(DecodeError::UnknownDiscriminator { .. }) => {}
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

    decoded.sort_by_key(|event| {
        (
            event.decoded.coordinate.instruction_index,
            event.decoded.coordinate.event_index,
        )
    });
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

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, time::Duration};

    use base64::{Engine as _, engine::general_purpose::STANDARD};
    use soldisco_solana_rpc::TransactionInstructionRecord;
    use soldisco_source_pump::{
        ANCHOR_EVENT_CPI_DISCRIMINATOR, COMPLETE_EVENT_DISCRIMINATOR, PUMP_PROGRAM_ID, PumpEvent,
        PumpProgram,
    };

    use super::{
        PIPELINE_RESTART_MAX_DELAY, PipelineError, ProgramLogBatch, SOLANA_DEVNET_GENESIS_HASH,
        SOLANA_MAINNET_GENESIS_HASH, decode_batch_events, next_restart_delay,
        pipeline_error_is_retryable, prune_flow,
    };
    use crate::jobs::maintenance::MaintenanceError;

    fn complete_event_bytes(marker: u8) -> Vec<u8> {
        let mut bytes = COMPLETE_EVENT_DISCRIMINATOR.to_vec();
        bytes.extend([marker; 32]);
        bytes.extend([marker.saturating_add(1); 32]);
        bytes.extend([marker.saturating_add(2); 32]);
        bytes.extend(1_720_000_000_i64.to_le_bytes());
        bytes.extend([marker.saturating_add(3); 32]);
        bytes
    }

    fn batch(
        instructions: Vec<TransactionInstructionRecord>,
        direct_event: Vec<u8>,
    ) -> ProgramLogBatch {
        ProgramLogBatch {
            program: PumpProgram::Pump,
            slot: 42,
            transaction_index: Some(3),
            signature: "signature".to_owned(),
            received_time_unix_ms: 1_720_000_000_000,
            instructions,
            log_messages: vec![
                format!("Program {PUMP_PROGRAM_ID} invoke [1]"),
                format!("Program data: {}", STANDARD.encode(direct_event)),
                format!("Program {PUMP_PROGRAM_ID} success"),
            ],
            transaction_error: None,
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
    fn authoritative_cpi_event_replaces_its_direct_log_copy() {
        let mut cpi_data = ANCHOR_EVENT_CPI_DISCRIMINATOR.to_vec();
        cpi_data.extend(complete_event_bytes(1));
        let decoded = decode_batch_events(&batch(
            vec![TransactionInstructionRecord {
                outer_instruction_index: 0,
                inner_instruction_index: Some(2),
                stack_height: Some(2),
                program_id: PUMP_PROGRAM_ID.to_owned(),
                data: cpi_data,
            }],
            complete_event_bytes(1),
        ))
        .expect("current CPI event should decode");
        let events = decoded.events;

        assert_eq!(events.len(), 1);
        assert!(decoded.quarantines.is_empty());
        assert!(matches!(events[0].decoded.event, PumpEvent::Complete(_)));
        assert_eq!(events[0].decoded.coordinate.event_index, 0);
        assert_eq!(events[0].raw_evidence[..8], ANCHOR_EVENT_CPI_DISCRIMINATOR);
    }

    #[test]
    fn direct_program_data_is_used_when_instruction_has_no_cpi_event() {
        let evidence = complete_event_bytes(1);
        let decoded = decode_batch_events(&batch(Vec::new(), evidence.clone()))
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
        let decoded = decode_batch_events(&batch(
            vec![TransactionInstructionRecord {
                outer_instruction_index: 0,
                inner_instruction_index: Some(2),
                stack_height: Some(2),
                program_id: PUMP_PROGRAM_ID.to_owned(),
                data: cpi_data,
            }],
            complete_event_bytes(10),
        ))
        .expect("mixed direct and CPI events should decode");
        let events = decoded.events;

        assert_eq!(events.len(), 2);
        assert!(decoded.quarantines.is_empty());
        assert_eq!(events[0].decoded.coordinate.event_index, 0);
        assert_eq!(events[1].decoded.coordinate.event_index, 1);
        assert_ne!(events[0].decoded.event, events[1].decoded.event);
    }

    #[test]
    fn malformed_known_event_is_quarantined_without_poisoning_the_batch() {
        let decoded =
            decode_batch_events(&batch(Vec::new(), COMPLETE_EVENT_DISCRIMINATOR.to_vec()))
                .expect("malformed evidence should be isolated");

        assert!(decoded.events.is_empty());
        assert_eq!(decoded.quarantines.len(), 1);
        assert_eq!(decoded.quarantines[0].reason_code, "MALFORMED_PUMP_EVENT");
    }
}
