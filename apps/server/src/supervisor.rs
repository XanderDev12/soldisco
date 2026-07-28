use std::sync::Arc;

use soldisco_api_contracts::{LiveEvent, StreamCommandResponse, StreamStateResponse, StreamStatus};
use soldisco_persistence::{Database, PersistenceError};
use thiserror::Error;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;

use crate::state::LiveEventBus;

#[derive(Clone)]
pub struct StreamSupervisor {
    database: Database,
    events: LiveEventBus,
    status: Arc<RwLock<StreamStatus>>,
    shutdown: CancellationToken,
}

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("the Pump collector is not implemented in the backend-foundation milestone")]
    CollectorUnavailable,
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
}

impl StreamSupervisor {
    pub async fn restore(
        database: Database,
        events: LiveEventBus,
    ) -> Result<Self, PersistenceError> {
        let requested_running = database.stream_requested_running().await?;
        let status = if requested_running {
            StreamStatus::Degraded
        } else {
            StreamStatus::Stopped
        };

        Ok(Self {
            database,
            events,
            status: Arc::new(RwLock::new(status)),
            shutdown: CancellationToken::new(),
        })
    }

    pub async fn state(&self) -> Result<StreamStateResponse, PersistenceError> {
        Ok(StreamStateResponse {
            status: *self.status.read().await,
            requested_running: self.database.stream_requested_running().await?,
        })
    }

    /// The command boundary exists now, but cannot claim a running collector
    /// until the Pump/PumpSwap intake milestone supplies one.
    pub async fn start(&self) -> Result<StreamCommandResponse, SupervisorError> {
        Err(SupervisorError::CollectorUnavailable)
    }

    pub async fn stop(&self) -> Result<StreamCommandResponse, SupervisorError> {
        let was_requested = self.database.stream_requested_running().await?;
        let previous = *self.status.read().await;
        self.database.set_stream_requested_running(false).await?;
        self.set_status(StreamStatus::Stopped).await;

        Ok(StreamCommandResponse {
            status: StreamStatus::Stopped,
            changed: was_requested || previous != StreamStatus::Stopped,
        })
    }

    pub async fn status(&self) -> StreamStatus {
        *self.status.read().await
    }

    pub fn shutdown(&self) {
        self.shutdown.cancel();
    }

    #[must_use]
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    async fn set_status(&self, status: StreamStatus) {
        let mut current = self.status.write().await;
        if *current == status {
            return;
        }

        *current = status;
        drop(current);
        self.events.publish(LiveEvent::StreamStatusChanged(status));
    }
}
