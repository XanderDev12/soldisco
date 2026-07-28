use serde::{Deserialize, Serialize};
use soldisco_domain::{AssessmentDecision, Score, Venue};

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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScreeningCounters {
    pub pending: u64,
    pub approved: u64,
    pub rejected: u64,
    pub flow_per_minute: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ApprovedToken {
    pub mint: String,
    pub symbol: Option<String>,
    pub primary_venue: Venue,
    pub deterministic_decision: AssessmentDecision,
    pub risk_score: Score,
    pub opportunity_score: Score,
    pub observed_slot: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RejectionSummary {
    pub reason_code: String,
    pub count: u64,
    pub last_seen_unix_ms: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoverySnapshot {
    pub sequence: u64,
    pub approved_tokens: Vec<ApprovedToken>,
    pub counters: ScreeningCounters,
    pub rejection_reasons: Vec<RejectionSummary>,
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
    use super::{LiveEnvelope, LiveEvent};

    #[test]
    fn live_event_is_a_tagged_browser_contract() {
        let encoded = serde_json::to_value(LiveEnvelope {
            sequence: 7,
            event: LiveEvent::DiscoveryProjectionChanged,
        })
        .expect("contract should serialize");

        assert_eq!(encoded["event"]["type"], "DISCOVERY_PROJECTION_CHANGED");
    }
}
