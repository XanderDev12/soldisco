use std::sync::Arc;

use soldisco_api_contracts::{LiveEvent, StreamCommandResponse, StreamStateResponse, StreamStatus};
use soldisco_persistence::{Database, PersistenceError};
use thiserror::Error;
use tokio::{
    sync::{Mutex, RwLock},
    task::{AbortHandle, JoinHandle},
};
use tokio_util::sync::CancellationToken;
use tracing::error;

use crate::{
    jobs::{
        discovery_rpc::DiscoveryRpcGate,
        pipeline::{PipelineConfig, PipelineError, SpawnedPipeline, spawn_pipeline},
    },
    state::LiveEventBus,
};

#[derive(Clone)]
pub struct StreamSupervisor {
    database: Database,
    events: LiveEventBus,
    status: Arc<RwLock<StreamStatus>>,
    shutdown: CancellationToken,
    pipeline_config: PipelineConfig,
    discovery_rpc: DiscoveryRpcGate,
    start_timeout: std::time::Duration,
    command_lock: Arc<Mutex<()>>,
    running: Arc<Mutex<Option<RunningPipeline>>>,
}

struct RunningPipeline {
    cancellation: CancellationToken,
    abort: AbortHandle,
    monitor: JoinHandle<()>,
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("the discovery stream is shutting down")]
    ShuttingDown,
    #[error(transparent)]
    Pipeline(#[from] PipelineError),
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

impl StreamSupervisor {
    pub async fn restore(
        database: Database,
        events: LiveEventBus,
        pipeline_config: PipelineConfig,
        start_timeout: std::time::Duration,
    ) -> Result<Self, PersistenceError> {
        let requested_running = database.stream_requested_running().await?;
        let status = if requested_running {
            StreamStatus::Degraded
        } else {
            StreamStatus::Stopped
        };
        let discovery_rpc = pipeline_config.discovery_rpc_gate();

        Ok(Self {
            database,
            events,
            status: Arc::new(RwLock::new(status)),
            shutdown: CancellationToken::new(),
            pipeline_config,
            discovery_rpc,
            start_timeout,
            command_lock: Arc::new(Mutex::new(())),
            running: Arc::new(Mutex::new(None)),
        })
    }

    pub async fn resume_if_requested(&self) -> Result<(), SupervisorError> {
        if self.database.stream_requested_running().await? {
            self.launch_pipeline().await?;
        }
        Ok(())
    }

    pub async fn state(&self) -> Result<StreamStateResponse, PersistenceError> {
        Ok(StreamStateResponse {
            status: *self.status.read().await,
            requested_running: self.database.stream_requested_running().await?,
        })
    }

    pub async fn start(&self) -> Result<StreamCommandResponse, SupervisorError> {
        let _command = self.command_lock.lock().await;
        if self.shutdown.is_cancelled() {
            return Err(SupervisorError::ShuttingDown);
        }

        let was_requested = self.database.stream_requested_running().await?;
        self.database.set_stream_requested_running(true).await?;
        let launched = self.launch_pipeline().await?;
        let status = self.wait_for_start_result().await;

        Ok(StreamCommandResponse {
            status,
            changed: launched || !was_requested,
        })
    }

    pub async fn stop(&self) -> Result<StreamCommandResponse, SupervisorError> {
        let _command = self.command_lock.lock().await;
        let was_requested = self.database.stream_requested_running().await?;
        let previous = *self.status.read().await;
        self.database.set_stream_requested_running(false).await?;
        let running = self.running.lock().await.take();
        let had_running = running.is_some();

        if let Some(running) = running {
            self.set_status(StreamStatus::Stopping).await;
            running.cancellation.cancel();
            await_monitor(running.monitor, running.abort, self.start_timeout).await;
        }
        self.set_status(StreamStatus::Stopped).await;

        Ok(StreamCommandResponse {
            status: StreamStatus::Stopped,
            changed: was_requested || previous != StreamStatus::Stopped || had_running,
        })
    }

    pub async fn status(&self) -> StreamStatus {
        *self.status.read().await
    }

    pub fn shutdown(&self) {
        self.shutdown.cancel();
        if let Ok(running) = self.running.try_lock()
            && let Some(running) = running.as_ref()
        {
            running.cancellation.cancel();
        }
    }

    #[must_use]
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    async fn set_status(&self, status: StreamStatus) {
        set_status(&self.status, &self.events, status).await;
    }

    async fn launch_pipeline(&self) -> Result<bool, SupervisorError> {
        let mut running = self.running.lock().await;
        if let Some(existing) = running.as_ref()
            && !existing.monitor.is_finished()
        {
            return Ok(false);
        }
        if let Some(finished) = running.take() {
            let _ = finished.monitor.await;
        }

        self.set_status(StreamStatus::Starting).await;
        let spawned = match spawn_pipeline(
            self.database.clone(),
            self.events.clone(),
            self.pipeline_config.clone(),
            self.discovery_rpc.clone(),
        )
        .await
        {
            Ok(spawned) => spawned,
            Err(error) => {
                self.set_status(StreamStatus::Error).await;
                return Err(error.into());
            }
        };
        let cancellation = spawned.cancellation.clone();
        let abort = spawned.task.abort_handle();
        let monitor = spawn_monitor(
            spawned,
            self.status.clone(),
            self.events.clone(),
            self.shutdown.clone(),
        );
        *running = Some(RunningPipeline {
            cancellation,
            abort,
            monitor,
        });
        Ok(true)
    }

    async fn wait_for_start_result(&self) -> StreamStatus {
        let deadline = tokio::time::Instant::now() + self.start_timeout;
        loop {
            let status = self.status().await;
            if matches!(
                status,
                StreamStatus::Running | StreamStatus::Degraded | StreamStatus::Error
            ) {
                return status;
            }
            if tokio::time::Instant::now() >= deadline {
                return status;
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    }
}

fn spawn_monitor(
    mut spawned: SpawnedPipeline,
    status: Arc<RwLock<StreamStatus>>,
    events: LiveEventBus,
    shutdown: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut shutdown_seen = false;
        let mut status_open = true;
        loop {
            tokio::select! {
                () = shutdown.cancelled(), if !shutdown_seen => {
                    shutdown_seen = true;
                    spawned.cancellation.cancel();
                }
                changed = spawned.status.changed(), if status_open => {
                    if changed.is_ok() {
                        let next_status = *spawned.status.borrow();
                        set_status(&status, &events, next_status).await;
                    } else {
                        status_open = false;
                    }
                }
                result = &mut spawned.task => {
                    match result {
                        Ok(Ok(())) if spawned.cancellation.is_cancelled() => {
                            set_status(&status, &events, StreamStatus::Stopped).await;
                        }
                        Ok(Ok(())) => {
                            error!("discovery pipeline exited unexpectedly");
                            set_status(&status, &events, StreamStatus::Error).await;
                        }
                        Ok(Err(pipeline_error)) => {
                            error!(error = %pipeline_error, "discovery pipeline failed");
                            set_status(&status, &events, StreamStatus::Error).await;
                        }
                        Err(join_error) => {
                            error!(error = %join_error, "discovery pipeline task failed to join");
                            set_status(&status, &events, StreamStatus::Error).await;
                        }
                    }
                    return;
                }
            }
        }
    })
}

async fn set_status(
    current_status: &RwLock<StreamStatus>,
    events: &LiveEventBus,
    next_status: StreamStatus,
) {
    let mut current = current_status.write().await;
    if *current == next_status {
        return;
    }

    *current = next_status;
    drop(current);
    events.publish(LiveEvent::StreamStatusChanged(next_status));
}

async fn await_monitor(
    mut monitor: JoinHandle<()>,
    pipeline_abort: AbortHandle,
    timeout: std::time::Duration,
) {
    if tokio::time::timeout(timeout, &mut monitor).await.is_err() {
        pipeline_abort.abort();
        if tokio::time::timeout(std::time::Duration::from_secs(1), &mut monitor)
            .await
            .is_err()
        {
            monitor.abort();
            let _ = monitor.await;
        }
    }
}
