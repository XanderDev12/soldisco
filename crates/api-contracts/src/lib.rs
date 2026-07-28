use serde::{Deserialize, Serialize};
use soldisco_domain::{Score, SourceProgram, Venue};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StreamStatus {
    Stopped,
    Starting,
    Running,
    Degraded,
    Error,
    Stopping,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoveryMode {
    /// Every structurally valid decoded Pump or PumpSwap candidate is visible.
    ObserveAll,
    /// Only candidates with an explicit deterministic pass are visible.
    ApprovedOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoveryStage {
    Observed,
    Approved,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryCounters {
    pub observed: u64,
    pub pending: u64,
    pub approved: u64,
    pub rejected: u64,
    pub flow_per_minute: Option<u64>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryActivity {
    pub trades: u64,
    pub buys: u64,
    pub sells: u64,
    pub unique_traders: u64,
    /// Atomic units are strings because valid on-chain quantities can exceed
    /// JavaScript's lossless integer range.
    pub base_volume_units: String,
    pub quote_volume_units: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryToken {
    pub mint: String,
    pub name: Option<String>,
    pub symbol: Option<String>,
    pub primary_venue: Venue,
    pub market_address: String,
    pub quote_mint: Option<String>,
    pub source_program: SourceProgram,
    pub stage: DiscoveryStage,
    pub last_event_kind: String,
    pub observed_slot: u64,
    pub first_observed_unix_ms: i64,
    pub last_observed_unix_ms: i64,
    pub latest_signature: String,
    pub activity: DiscoveryActivity,
    pub risk_score: Option<Score>,
    pub opportunity_score: Option<Score>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RejectionSummary {
    pub reason_code: String,
    pub count: u64,
    pub last_seen_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoverySnapshot {
    pub sequence: u64,
    pub mode: DiscoveryMode,
    pub tokens: Vec<DiscoveryToken>,
    /// Total candidates visible under `mode`, before the bounded snapshot
    /// limit is applied.
    pub tokens_total: u64,
    pub tokens_truncated: bool,
    pub counters: DiscoveryCounters,
    pub rejection_reasons: Vec<RejectionSummary>,
}

impl Default for DiscoverySnapshot {
    fn default() -> Self {
        Self {
            sequence: 0,
            mode: DiscoveryMode::ObserveAll,
            tokens: Vec::new(),
            tokens_total: 0,
            tokens_truncated: false,
            counters: DiscoveryCounters::default(),
            rejection_reasons: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StreamCommandResponse {
    pub status: StreamStatus,
    pub changed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StreamStateResponse {
    pub status: StreamStatus,
    pub requested_running: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub database: String,
    pub stream: StreamStatus,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LiveEnvelope {
    pub sequence: u64,
    pub event: LiveEvent,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiveEvent {
    StreamStatusChanged(StreamStatus),
    DiscoveryProjectionChanged,
    ResyncRequired,
}

#[cfg(test)]
mod tests {
    use super::{DiscoveryMode, DiscoverySnapshot, LiveEnvelope, LiveEvent};

    #[test]
    fn live_event_is_a_tagged_browser_contract() {
        let encoded = serde_json::to_value(LiveEnvelope {
            sequence: 7,
            event: LiveEvent::DiscoveryProjectionChanged,
        })
        .expect("contract should serialize");

        assert_eq!(encoded["event"]["type"], "DISCOVERY_PROJECTION_CHANGED");
    }

    #[test]
    fn empty_discovery_is_explicitly_unfiltered() {
        let snapshot = DiscoverySnapshot::default();

        assert_eq!(snapshot.mode, DiscoveryMode::ObserveAll);
        assert!(snapshot.tokens.is_empty());
        assert_eq!(snapshot.counters.flow_per_minute, None);
    }
}
