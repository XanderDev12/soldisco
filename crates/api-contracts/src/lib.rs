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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SettingsApplyRequirement {
    StreamRestart,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct IntegerBounds {
    pub minimum: u64,
    pub maximum: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PrefilterDefaultsBounds {
    pub max_event_age_ms: IntegerBounds,
    pub observation_window_ms: IntegerBounds,
    pub max_active_windows: IntegerBounds,
    pub rpc_requests_per_second: IntegerBounds,
    pub rpc_max_in_flight: IntegerBounds,
    pub rpc_request_timeout_ms: IntegerBounds,
    pub rpc_rate_limit_cooldown_ms: IntegerBounds,
}

impl PrefilterDefaultsBounds {
    pub const SUPPORTED: Self = Self {
        max_event_age_ms: IntegerBounds {
            minimum: 1_000,
            maximum: 300_000,
        },
        observation_window_ms: IntegerBounds {
            minimum: 1_000,
            maximum: 3_600_000,
        },
        max_active_windows: IntegerBounds {
            minimum: 1,
            maximum: 100_000,
        },
        rpc_requests_per_second: IntegerBounds {
            minimum: 1,
            maximum: 1_000,
        },
        rpc_max_in_flight: IntegerBounds {
            minimum: 1,
            maximum: 128,
        },
        rpc_request_timeout_ms: IntegerBounds {
            minimum: 1,
            maximum: 300_000,
        },
        rpc_rate_limit_cooldown_ms: IntegerBounds {
            minimum: 100,
            maximum: 300_000,
        },
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PrefilterDefaults {
    pub max_event_age_ms: u64,
    pub observation_window_ms: u64,
    pub max_active_windows: u32,
    pub rpc_requests_per_second: u32,
    pub rpc_max_in_flight: u32,
    pub rpc_request_timeout_ms: u64,
    pub rpc_rate_limit_cooldown_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrefilterDefaultsValidationError {
    OutOfBounds {
        field: &'static str,
        value: u64,
        bounds: IntegerBounds,
    },
    RpcTimeoutExceedsMaximumEventAge,
    ObservationWindowShorterThanRpcTimeout,
}

impl PrefilterDefaultsValidationError {
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::OutOfBounds { field, .. } => field,
            Self::RpcTimeoutExceedsMaximumEventAge => "rpc_request_timeout_ms",
            Self::ObservationWindowShorterThanRpcTimeout => "observation_window_ms",
        }
    }
}

impl std::fmt::Display for PrefilterDefaultsValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfBounds {
                field,
                value,
                bounds,
            } => write!(
                formatter,
                "{field} value {value} is outside {} through {}",
                bounds.minimum, bounds.maximum
            ),
            Self::RpcTimeoutExceedsMaximumEventAge => {
                formatter.write_str("rpc_request_timeout_ms cannot exceed max_event_age_ms")
            }
            Self::ObservationWindowShorterThanRpcTimeout => formatter
                .write_str("observation_window_ms cannot be shorter than rpc_request_timeout_ms"),
        }
    }
}

impl std::error::Error for PrefilterDefaultsValidationError {}

impl PrefilterDefaults {
    pub fn validate(self) -> Result<(), PrefilterDefaultsValidationError> {
        let bounds = PrefilterDefaultsBounds::SUPPORTED;
        validate_integer(
            "max_event_age_ms",
            self.max_event_age_ms,
            bounds.max_event_age_ms,
        )?;
        validate_integer(
            "observation_window_ms",
            self.observation_window_ms,
            bounds.observation_window_ms,
        )?;
        validate_integer(
            "max_active_windows",
            u64::from(self.max_active_windows),
            bounds.max_active_windows,
        )?;
        validate_integer(
            "rpc_requests_per_second",
            u64::from(self.rpc_requests_per_second),
            bounds.rpc_requests_per_second,
        )?;
        validate_integer(
            "rpc_max_in_flight",
            u64::from(self.rpc_max_in_flight),
            bounds.rpc_max_in_flight,
        )?;
        validate_integer(
            "rpc_request_timeout_ms",
            self.rpc_request_timeout_ms,
            bounds.rpc_request_timeout_ms,
        )?;
        validate_integer(
            "rpc_rate_limit_cooldown_ms",
            self.rpc_rate_limit_cooldown_ms,
            bounds.rpc_rate_limit_cooldown_ms,
        )?;
        if self.rpc_request_timeout_ms > self.max_event_age_ms {
            return Err(PrefilterDefaultsValidationError::RpcTimeoutExceedsMaximumEventAge);
        }
        if self.observation_window_ms < self.rpc_request_timeout_ms {
            return Err(PrefilterDefaultsValidationError::ObservationWindowShorterThanRpcTimeout);
        }
        Ok(())
    }
}

fn validate_integer(
    field: &'static str,
    value: u64,
    bounds: IntegerBounds,
) -> Result<(), PrefilterDefaultsValidationError> {
    if (bounds.minimum..=bounds.maximum).contains(&value) {
        Ok(())
    } else {
        Err(PrefilterDefaultsValidationError::OutOfBounds {
            field,
            value,
            bounds,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PrefilterDefaultsResponse {
    pub values: PrefilterDefaults,
    pub bounds: PrefilterDefaultsBounds,
    pub revision: u64,
    pub apply_requirement: SettingsApplyRequirement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UpdatePrefilterDefaultsRequest {
    pub expected_revision: u64,
    pub values: PrefilterDefaults,
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
    use super::{
        DiscoveryMode, DiscoverySnapshot, LiveEnvelope, LiveEvent, PrefilterDefaults,
        PrefilterDefaultsBounds, PrefilterDefaultsResponse, PrefilterDefaultsValidationError,
        SettingsApplyRequirement, UpdatePrefilterDefaultsRequest,
    };

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

    fn defaults() -> PrefilterDefaults {
        PrefilterDefaults {
            max_event_age_ms: 15_000,
            observation_window_ms: 60_000,
            max_active_windows: 128,
            rpc_requests_per_second: 1,
            rpc_max_in_flight: 4,
            rpc_request_timeout_ms: 5_000,
            rpc_rate_limit_cooldown_ms: 5_000,
        }
    }

    #[test]
    fn prefilter_settings_contract_uses_stable_browser_names() {
        let response = PrefilterDefaultsResponse {
            values: defaults(),
            bounds: PrefilterDefaultsBounds::SUPPORTED,
            revision: 3,
            apply_requirement: SettingsApplyRequirement::StreamRestart,
        };
        let encoded = serde_json::to_value(response).expect("contract should serialize");

        assert_eq!(encoded["revision"], 3);
        assert_eq!(encoded["values"]["max_event_age_ms"], 15_000);
        assert_eq!(encoded["bounds"]["max_event_age_ms"]["minimum"], 1_000);
        assert_eq!(encoded["apply_requirement"], "STREAM_RESTART");

        let update = serde_json::to_value(UpdatePrefilterDefaultsRequest {
            expected_revision: 3,
            values: defaults(),
        })
        .expect("update should serialize");
        assert!(update.get("values").is_some());
        assert!(update.get("defaults").is_none());
    }

    #[test]
    fn prefilter_settings_reject_unsafe_relationships_and_bounds() {
        let mut values = defaults();
        values.rpc_request_timeout_ms = values.max_event_age_ms + 1;
        assert_eq!(
            values.validate(),
            Err(PrefilterDefaultsValidationError::RpcTimeoutExceedsMaximumEventAge)
        );

        let mut values = defaults();
        values.observation_window_ms = values.rpc_request_timeout_ms - 1;
        assert_eq!(
            values.validate(),
            Err(PrefilterDefaultsValidationError::ObservationWindowShorterThanRpcTimeout)
        );

        let mut values = defaults();
        values.rpc_max_in_flight = 0;
        assert!(matches!(
            values.validate(),
            Err(PrefilterDefaultsValidationError::OutOfBounds {
                field: "rpc_max_in_flight",
                ..
            })
        ));
    }
}
