use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::Duration;

use axum::http::{
    HeaderMap,
    header::{HOST, ORIGIN},
};
use soldisco_api_contracts::{
    DiscoverySnapshot, DiscoveryToken, LiveEnvelope, LiveEvent, PrefilterDefaults,
    PrefilterDefaultsBounds, PrefilterDefaultsResponse, QualificationDefaults,
    QualificationDefaultsBounds, QualificationDefaultsResponse, SettingsApplyRequirement,
};
use soldisco_persistence::{Database, PersistenceError};
use tokio::sync::broadcast;
use tracing::error;

use crate::{
    config::Config,
    jobs::{pipeline::PipelineConfig, qualification::finalize_abandoned_windows},
    supervisor::{StreamSupervisor, SupervisorError},
};

const LIVE_EVENT_BUFFER: usize = 256;
const DISCOVERY_INVALIDATION_DELAY: Duration = Duration::from_millis(250);
pub const LOCAL_CONTROL_HEADER_NAME: &str = "x-soldisco-control";
pub const LOCAL_CONTROL_HEADER_VALUE: &str = "soldisco-local-ui-v1";

#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    database: Database,
    events: LiveEventBus,
    supervisor: StreamSupervisor,
    discovery_snapshot_limit: u32,
    web_origin: String,
    api_authority: String,
}

#[derive(Clone)]
pub struct LiveEventBus {
    sender: broadcast::Sender<LiveEnvelope>,
    sequence: Arc<AtomicU64>,
    discovery_invalidation_pending: Arc<AtomicBool>,
}

impl AppState {
    pub async fn new(database: Database, config: &Config) -> Result<Self, PersistenceError> {
        database
            .initialize_prefilter_defaults(config.prefilter_defaults())
            .await?;
        database
            .initialize_qualification_defaults(QualificationDefaults::SAFE_INITIAL)
            .await?;
        let events = LiveEventBus::new();
        let recovery_run_id = format!("soldisco-startup-recovery-{}", std::process::id());
        match finalize_abandoned_windows(&database, &events, &recovery_run_id).await {
            Ok(recovered) if recovered > 0 => {
                tracing::warn!(
                    windows = recovered,
                    "finalized interrupted discovery windows during startup recovery"
                );
            }
            Ok(_) => {}
            Err(recovery_error) => {
                error!(
                    error = %recovery_error,
                    "startup discovery-window recovery could not finish; runtime retry remains available"
                );
            }
        }
        let supervisor = StreamSupervisor::restore(
            database.clone(),
            events.clone(),
            PipelineConfig::from(config),
            config.stream_start_timeout,
        )
        .await?;

        let state = Self {
            inner: Arc::new(Inner {
                database,
                events,
                supervisor,
                discovery_snapshot_limit: config.discovery_snapshot_limit,
                web_origin: config.web_origin.clone(),
                api_authority: config.api_address.to_string(),
            }),
        };
        if let Err(resume_error) = state.inner.supervisor.resume_if_requested().await {
            error!(error = %resume_error, "failed to resume the requested discovery stream");
        }
        Ok(state)
    }

    #[must_use]
    pub fn database(&self) -> &Database {
        &self.inner.database
    }

    #[must_use]
    pub fn supervisor(&self) -> &StreamSupervisor {
        &self.inner.supervisor
    }

    pub async fn discovery_snapshot(&self) -> Result<DiscoverySnapshot, PersistenceError> {
        self.inner
            .database
            .load_discovery_snapshot_bounded(self.inner.discovery_snapshot_limit)
            .await
    }

    pub async fn discovery_token(
        &self,
        mint: &str,
    ) -> Result<Option<DiscoveryToken>, PersistenceError> {
        self.inner.database.load_discovery_token(mint).await
    }

    pub async fn prefilter_defaults(&self) -> Result<PrefilterDefaultsResponse, PersistenceError> {
        let stored = self.inner.supervisor.prefilter_defaults().await?;
        Ok(prefilter_defaults_response(stored))
    }

    pub async fn update_prefilter_defaults(
        &self,
        expected_revision: u64,
        values: PrefilterDefaults,
    ) -> Result<PrefilterDefaultsResponse, SupervisorError> {
        let stored = self
            .inner
            .supervisor
            .update_prefilter_defaults(expected_revision, values)
            .await?;
        Ok(prefilter_defaults_response(stored))
    }

    pub async fn qualification_defaults(
        &self,
    ) -> Result<QualificationDefaultsResponse, PersistenceError> {
        let stored = self.inner.database.load_qualification_defaults().await?;
        Ok(qualification_defaults_response(stored))
    }

    pub async fn update_qualification_defaults(
        &self,
        expected_revision: u64,
        values: QualificationDefaults,
    ) -> Result<QualificationDefaultsResponse, PersistenceError> {
        let stored = self
            .inner
            .database
            .update_qualification_defaults(expected_revision, values)
            .await?;
        Ok(qualification_defaults_response(stored))
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

    #[must_use]
    pub fn local_control_authorized(&self, headers: &HeaderMap) -> bool {
        local_control_headers_authorized(headers, &self.inner.web_origin, &self.inner.api_authority)
    }
}

fn prefilter_defaults_response(
    stored: soldisco_persistence::StoredPrefilterDefaults,
) -> PrefilterDefaultsResponse {
    PrefilterDefaultsResponse {
        values: stored.values,
        bounds: PrefilterDefaultsBounds::SUPPORTED,
        revision: stored.revision,
        apply_requirement: SettingsApplyRequirement::StreamRestart,
    }
}

fn qualification_defaults_response(
    stored: soldisco_persistence::StoredQualificationDefaults,
) -> QualificationDefaultsResponse {
    QualificationDefaultsResponse {
        values: stored.values,
        bounds: QualificationDefaultsBounds::SUPPORTED,
        revision: stored.revision,
        apply_requirement: SettingsApplyRequirement::NewWindows,
    }
}

fn local_control_headers_authorized(
    headers: &HeaderMap,
    expected_origin: &str,
    expected_authority: &str,
) -> bool {
    let control_matches = headers
        .get(LOCAL_CONTROL_HEADER_NAME)
        .and_then(|value| value.to_str().ok())
        == Some(LOCAL_CONTROL_HEADER_VALUE);
    let host_matches =
        headers.get(HOST).and_then(|value| value.to_str().ok()) == Some(expected_authority);
    if !control_matches || !host_matches {
        return false;
    }

    match headers.get(ORIGIN) {
        None => true,
        Some(origin) => origin
            .to_str()
            .is_ok_and(|origin| origin == expected_origin),
    }
}

impl LiveEventBus {
    fn new() -> Self {
        let (sender, _) = broadcast::channel(LIVE_EVENT_BUFFER);
        Self {
            sender,
            sequence: Arc::new(AtomicU64::new(0)),
            discovery_invalidation_pending: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn publish(&self, event: LiveEvent) {
        let envelope = self.envelope(event);
        let _ = self.sender.send(envelope);
    }

    /// Coalesces projection invalidations so a busy discovery stream cannot
    /// force every connected browser to refetch the full snapshot once per
    /// observation. Stream-status events remain immediate.
    pub fn publish_discovery_projection_changed(&self) {
        if self
            .discovery_invalidation_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }

        let events = self.clone();
        tokio::spawn(async move {
            tokio::time::sleep(DISCOVERY_INVALIDATION_DELAY).await;
            events
                .discovery_invalidation_pending
                .store(false, Ordering::Release);
            events.publish(LiveEvent::DiscoveryProjectionChanged);
        });
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::{
        HeaderMap, HeaderValue,
        header::{HOST, ORIGIN},
    };
    use soldisco_api_contracts::{LiveEvent, StreamStatus};

    use super::{
        LOCAL_CONTROL_HEADER_NAME, LOCAL_CONTROL_HEADER_VALUE, LiveEventBus,
        local_control_headers_authorized,
    };

    #[tokio::test]
    async fn projection_invalidations_are_coalesced_but_status_is_immediate() {
        let events = LiveEventBus::new();
        let mut receiver = events.subscribe();

        events.publish_discovery_projection_changed();
        events.publish_discovery_projection_changed();
        events.publish(LiveEvent::StreamStatusChanged(StreamStatus::Running));

        let immediate = tokio::time::timeout(Duration::from_millis(50), receiver.recv())
            .await
            .expect("status event should be immediate")
            .expect("event channel should remain open");
        assert_eq!(
            immediate.event,
            LiveEvent::StreamStatusChanged(StreamStatus::Running)
        );

        let invalidation = tokio::time::timeout(Duration::from_secs(1), receiver.recv())
            .await
            .expect("coalesced invalidation should be delivered")
            .expect("event channel should remain open");
        assert_eq!(invalidation.event, LiveEvent::DiscoveryProjectionChanged);

        assert!(
            tokio::time::timeout(Duration::from_millis(100), receiver.recv())
                .await
                .is_err(),
            "the duplicate invalidation should be coalesced"
        );
    }

    #[test]
    fn local_control_requires_exact_header_and_rejects_foreign_origins() {
        let expected_origin = "http://localhost:3000";
        let expected_authority = "127.0.0.1:8080";
        let mut headers = HeaderMap::new();
        assert!(!local_control_headers_authorized(
            &headers,
            expected_origin,
            expected_authority
        ));

        headers.insert(
            LOCAL_CONTROL_HEADER_NAME,
            HeaderValue::from_static(LOCAL_CONTROL_HEADER_VALUE),
        );
        assert!(!local_control_headers_authorized(
            &headers,
            expected_origin,
            expected_authority
        ));
        headers.insert(HOST, HeaderValue::from_static(expected_authority));
        assert!(local_control_headers_authorized(
            &headers,
            expected_origin,
            expected_authority
        ));

        headers.insert(ORIGIN, HeaderValue::from_static("https://attacker.invalid"));
        assert!(!local_control_headers_authorized(
            &headers,
            expected_origin,
            expected_authority
        ));

        headers.insert(
            ORIGIN,
            HeaderValue::from_bytes(b"\xff").expect("opaque header bytes"),
        );
        assert!(!local_control_headers_authorized(
            &headers,
            expected_origin,
            expected_authority
        ));

        headers.insert(ORIGIN, HeaderValue::from_static(expected_origin));
        assert!(local_control_headers_authorized(
            &headers,
            expected_origin,
            expected_authority
        ));

        headers.insert(HOST, HeaderValue::from_static("attacker.invalid"));
        assert!(!local_control_headers_authorized(
            &headers,
            expected_origin,
            expected_authority
        ));
    }
}
