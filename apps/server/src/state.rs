use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use soldisco_api_contracts::{ApprovedToken, DiscoverySnapshot, LiveEnvelope, LiveEvent};
use soldisco_persistence::{Database, PersistenceError};
use soldisco_projections::DiscoveryProjection;
use tokio::sync::{RwLock, broadcast};

use crate::supervisor::StreamSupervisor;

const LIVE_EVENT_BUFFER: usize = 256;

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    database: Database,
    discovery: RwLock<DiscoveryProjection>,
    events: LiveEventBus,
    supervisor: StreamSupervisor,
}

#[derive(Clone)]
pub struct LiveEventBus {
    sender: broadcast::Sender<LiveEnvelope>,
    sequence: Arc<AtomicU64>,
}

impl AppState {
    pub async fn new(database: Database) -> Result<Self, PersistenceError> {
        let events = LiveEventBus::new();
        let supervisor = StreamSupervisor::restore(database.clone(), events.clone()).await?;

        Ok(Self {
            inner: Arc::new(Inner {
                database,
                discovery: RwLock::new(DiscoveryProjection::default()),
                events,
                supervisor,
            }),
        })
    }

    #[must_use]
    pub fn database(&self) -> &Database {
        &self.inner.database
    }

    #[must_use]
    pub fn supervisor(&self) -> &StreamSupervisor {
        &self.inner.supervisor
    }

    pub async fn discovery_snapshot(&self) -> DiscoverySnapshot {
        self.inner.discovery.read().await.snapshot()
    }

    pub async fn approved_token(&self, mint: &str) -> Option<ApprovedToken> {
        self.discovery_snapshot()
            .await
            .approved_tokens
            .into_iter()
            .find(|token| token.mint == mint)
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<LiveEnvelope> {
        self.inner.events.subscribe()
    }

    #[must_use]
    pub fn resync_event(&self) -> LiveEnvelope {
        self.inner.events.resync_envelope()
    }

    #[must_use]
    pub fn shutdown_token(&self) -> tokio_util::sync::CancellationToken {
        self.inner.supervisor.shutdown_token()
    }

    pub fn shutdown(&self) {
        self.inner.supervisor.shutdown();
    }
}

impl LiveEventBus {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(LIVE_EVENT_BUFFER);
        Self {
            sender,
            sequence: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn publish(&self, event: LiveEvent) {
        let envelope = self.envelope(event);
        let _ = self.sender.send(envelope);
    }

    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<LiveEnvelope> {
        self.sender.subscribe()
    }

    #[must_use]
    pub fn envelope(&self, event: LiveEvent) -> LiveEnvelope {
        LiveEnvelope {
            sequence: self.sequence.fetch_add(1, Ordering::Relaxed) + 1,
            event,
        }
    }

    #[must_use]
    pub fn resync_envelope(&self) -> LiveEnvelope {
        LiveEnvelope {
            sequence: self.sequence.load(Ordering::Relaxed),
            event: LiveEvent::ResyncRequired,
        }
    }
}
