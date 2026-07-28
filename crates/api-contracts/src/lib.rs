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
    /// Only candidates that passed the bounded discovery-window qualification.
    QualifiedOnly,
    /// Only candidates with an explicit deterministic pass are visible.
    ApprovedOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoveryStage {
    Observed,
    Qualified,
    Approved,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryCounters {
    pub observed: u64,
    /// Legacy aggregate retained for browser compatibility.
    pub pending: u64,
    pub qualified: u64,
    pub qualification_pending: u64,
    pub qualification_rejected: u64,
    pub qualification_unknown: u64,
    pub processing_failures: u64,
    pub approved: u64,
    /// Legacy aggregate retained for browser compatibility.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QualificationDecision {
    Pass,
    Reject,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WindowCompleteness {
    Complete,
    Incomplete,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DiscoveryWindowSummary {
    pub window_revision: u64,
    pub ruleset_revision: u64,
    pub opened_unix_ms: i64,
    pub closed_unix_ms: i64,
    pub evaluated_unix_ms: i64,
    pub decision: QualificationDecision,
    pub completeness: WindowCompleteness,
    pub reason_codes: Vec<String>,
    pub trades: u64,
    pub buys: u64,
    pub sells: u64,
    pub unique_traders: u64,
    pub unique_buyers: u64,
    pub unique_sellers: u64,
    /// Atomic units are strings because valid on-chain quantities can exceed
    /// JavaScript's lossless integer range.
    pub buy_base_volume_units: String,
    pub sell_base_volume_units: String,
    pub buy_quote_volume_units: String,
    pub sell_quote_volume_units: String,
    pub maximum_single_wallet_quote_share_bps: Option<u16>,
    pub price_change_bps: Option<i64>,
    pub first_base_reserve_units: Option<String>,
    pub first_quote_reserve_units: Option<String>,
    pub latest_base_reserve_units: Option<String>,
    pub latest_quote_reserve_units: Option<String>,
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
    #[serde(default)]
    pub qualification: Option<DiscoveryWindowSummary>,
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
    NewWindows,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QualificationDefaultsBounds {
    pub minimum_trades: IntegerBounds,
    pub minimum_unique_traders: IntegerBounds,
    pub minimum_buys: IntegerBounds,
    pub minimum_sells: IntegerBounds,
    pub minimum_native_quote_volume_units: IntegerBounds,
    pub minimum_stable_quote_volume_units: IntegerBounds,
    pub maximum_single_wallet_quote_share_bps: IntegerBounds,
}

impl QualificationDefaultsBounds {
    pub const SUPPORTED: Self = Self {
        minimum_trades: IntegerBounds {
            minimum: 1,
            maximum: 10_000,
        },
        minimum_unique_traders: IntegerBounds {
            minimum: 1,
            maximum: 10_000,
        },
        minimum_buys: IntegerBounds {
            minimum: 0,
            maximum: 10_000,
        },
        minimum_sells: IntegerBounds {
            minimum: 0,
            maximum: 10_000,
        },
        minimum_native_quote_volume_units: IntegerBounds {
            minimum: 0,
            maximum: 9_007_199_254_740_991,
        },
        minimum_stable_quote_volume_units: IntegerBounds {
            minimum: 0,
            maximum: 9_007_199_254_740_991,
        },
        maximum_single_wallet_quote_share_bps: IntegerBounds {
            minimum: 1_000,
            maximum: 10_000,
        },
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QualificationDefaults {
    pub minimum_trades: u32,
    pub minimum_unique_traders: u32,
    pub minimum_buys: u32,
    pub minimum_sells: u32,
    pub minimum_native_quote_volume_units: u64,
    pub minimum_stable_quote_volume_units: u64,
    pub maximum_single_wallet_quote_share_bps: u16,
}

impl QualificationDefaults {
    pub const SAFE_INITIAL: Self = Self {
        minimum_trades: 5,
        minimum_unique_traders: 3,
        minimum_buys: 1,
        minimum_sells: 1,
        minimum_native_quote_volume_units: 50_000_000,
        minimum_stable_quote_volume_units: 5_000_000,
        maximum_single_wallet_quote_share_bps: 9_000,
    };

    pub fn validate(self) -> Result<(), QualificationDefaultsValidationError> {
        let bounds = QualificationDefaultsBounds::SUPPORTED;
        validate_qualification_integer(
            "minimum_trades",
            u64::from(self.minimum_trades),
            bounds.minimum_trades,
        )?;
        validate_qualification_integer(
            "minimum_unique_traders",
            u64::from(self.minimum_unique_traders),
            bounds.minimum_unique_traders,
        )?;
        validate_qualification_integer(
            "minimum_buys",
            u64::from(self.minimum_buys),
            bounds.minimum_buys,
        )?;
        validate_qualification_integer(
            "minimum_sells",
            u64::from(self.minimum_sells),
            bounds.minimum_sells,
        )?;
        validate_qualification_integer(
            "minimum_native_quote_volume_units",
            self.minimum_native_quote_volume_units,
            bounds.minimum_native_quote_volume_units,
        )?;
        validate_qualification_integer(
            "minimum_stable_quote_volume_units",
            self.minimum_stable_quote_volume_units,
            bounds.minimum_stable_quote_volume_units,
        )?;
        validate_qualification_integer(
            "maximum_single_wallet_quote_share_bps",
            u64::from(self.maximum_single_wallet_quote_share_bps),
            bounds.maximum_single_wallet_quote_share_bps,
        )?;

        if self.minimum_unique_traders > self.minimum_trades {
            return Err(
                QualificationDefaultsValidationError::MinimumUniqueTradersExceedMinimumTrades,
            );
        }
        if self.minimum_buys > self.minimum_trades {
            return Err(QualificationDefaultsValidationError::MinimumBuysExceedMinimumTrades);
        }
        if self.minimum_sells > self.minimum_trades {
            return Err(QualificationDefaultsValidationError::MinimumSellsExceedMinimumTrades);
        }

        Ok(())
    }
}

impl Default for QualificationDefaults {
    fn default() -> Self {
        Self::SAFE_INITIAL
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QualificationDefaultsValidationError {
    OutOfBounds {
        field: &'static str,
        value: u64,
        bounds: IntegerBounds,
    },
    MinimumUniqueTradersExceedMinimumTrades,
    MinimumBuysExceedMinimumTrades,
    MinimumSellsExceedMinimumTrades,
}

impl QualificationDefaultsValidationError {
    #[must_use]
    pub const fn field(self) -> &'static str {
        match self {
            Self::OutOfBounds { field, .. } => field,
            Self::MinimumUniqueTradersExceedMinimumTrades => "minimum_unique_traders",
            Self::MinimumBuysExceedMinimumTrades => "minimum_buys",
            Self::MinimumSellsExceedMinimumTrades => "minimum_sells",
        }
    }
}

impl std::fmt::Display for QualificationDefaultsValidationError {
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
            Self::MinimumUniqueTradersExceedMinimumTrades => {
                formatter.write_str("minimum_unique_traders cannot exceed minimum_trades")
            }
            Self::MinimumBuysExceedMinimumTrades => {
                formatter.write_str("minimum_buys cannot exceed minimum_trades")
            }
            Self::MinimumSellsExceedMinimumTrades => {
                formatter.write_str("minimum_sells cannot exceed minimum_trades")
            }
        }
    }
}

impl std::error::Error for QualificationDefaultsValidationError {}

fn validate_qualification_integer(
    field: &'static str,
    value: u64,
    bounds: IntegerBounds,
) -> Result<(), QualificationDefaultsValidationError> {
    if (bounds.minimum..=bounds.maximum).contains(&value) {
        Ok(())
    } else {
        Err(QualificationDefaultsValidationError::OutOfBounds {
            field,
            value,
            bounds,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct QualificationDefaultsResponse {
    pub values: QualificationDefaults,
    pub bounds: QualificationDefaultsBounds,
    pub revision: u64,
    pub apply_requirement: SettingsApplyRequirement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct UpdateQualificationDefaultsRequest {
    pub expected_revision: u64,
    pub values: QualificationDefaults,
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
        DiscoveryMode, DiscoverySnapshot, DiscoveryStage, DiscoveryToken, DiscoveryWindowSummary,
        LiveEnvelope, LiveEvent, PrefilterDefaults, PrefilterDefaultsBounds,
        PrefilterDefaultsResponse, PrefilterDefaultsValidationError, QualificationDecision,
        QualificationDefaults, QualificationDefaultsBounds, QualificationDefaultsResponse,
        QualificationDefaultsValidationError, SettingsApplyRequirement,
        UpdatePrefilterDefaultsRequest, UpdateQualificationDefaultsRequest, WindowCompleteness,
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

    #[test]
    fn qualification_settings_have_safe_valid_defaults() {
        let values = QualificationDefaults::default();

        assert_eq!(values, QualificationDefaults::SAFE_INITIAL);
        assert_eq!(values.minimum_trades, 5);
        assert_eq!(values.minimum_unique_traders, 3);
        assert_eq!(values.minimum_buys, 1);
        assert_eq!(values.minimum_sells, 1);
        assert_eq!(values.minimum_native_quote_volume_units, 50_000_000);
        assert_eq!(values.minimum_stable_quote_volume_units, 5_000_000);
        assert_eq!(values.maximum_single_wallet_quote_share_bps, 9_000);
        assert_eq!(values.validate(), Ok(()));
    }

    #[test]
    fn qualification_settings_contract_uses_stable_browser_names() {
        let response = QualificationDefaultsResponse {
            values: QualificationDefaults::default(),
            bounds: QualificationDefaultsBounds::SUPPORTED,
            revision: 4,
            apply_requirement: SettingsApplyRequirement::NewWindows,
        };
        let encoded = serde_json::to_value(response).expect("contract should serialize");

        assert_eq!(encoded["revision"], 4);
        assert_eq!(encoded["values"]["minimum_trades"], 5);
        assert_eq!(
            encoded["bounds"]["minimum_native_quote_volume_units"]["maximum"],
            9_007_199_254_740_991_u64
        );
        assert_eq!(encoded["apply_requirement"], "NEW_WINDOWS");

        let update = serde_json::to_value(UpdateQualificationDefaultsRequest {
            expected_revision: 4,
            values: QualificationDefaults::default(),
        })
        .expect("update should serialize");
        assert!(update.get("expected_revision").is_some());
        assert!(update.get("values").is_some());
        assert!(update.get("defaults").is_none());
    }

    #[test]
    fn qualification_settings_reject_bounds_and_impossible_relationships() {
        let values = QualificationDefaults {
            minimum_trades: 0,
            ..QualificationDefaults::default()
        };
        assert!(matches!(
            values.validate(),
            Err(QualificationDefaultsValidationError::OutOfBounds {
                field: "minimum_trades",
                ..
            })
        ));

        let values = QualificationDefaults {
            minimum_buys: 0,
            minimum_sells: 0,
            ..QualificationDefaults::default()
        };
        assert_eq!(values.validate(), Ok(()));

        let mut values = QualificationDefaults::default();
        values.minimum_unique_traders = values.minimum_trades + 1;
        assert_eq!(
            values.validate(),
            Err(QualificationDefaultsValidationError::MinimumUniqueTradersExceedMinimumTrades)
        );

        let mut values = QualificationDefaults::default();
        values.minimum_buys = values.minimum_trades + 1;
        assert_eq!(
            values.validate(),
            Err(QualificationDefaultsValidationError::MinimumBuysExceedMinimumTrades)
        );

        let mut values = QualificationDefaults::default();
        values.minimum_sells = values.minimum_trades + 1;
        assert_eq!(
            values.validate(),
            Err(QualificationDefaultsValidationError::MinimumSellsExceedMinimumTrades)
        );

        let values = QualificationDefaults {
            minimum_native_quote_volume_units: 9_007_199_254_740_992,
            ..QualificationDefaults::default()
        };
        assert!(matches!(
            values.validate(),
            Err(QualificationDefaultsValidationError::OutOfBounds {
                field: "minimum_native_quote_volume_units",
                ..
            })
        ));

        let values = QualificationDefaults {
            maximum_single_wallet_quote_share_bps: 999,
            ..QualificationDefaults::default()
        };
        assert!(matches!(
            values.validate(),
            Err(QualificationDefaultsValidationError::OutOfBounds {
                field: "maximum_single_wallet_quote_share_bps",
                ..
            })
        ));
    }

    #[test]
    fn qualification_result_contract_uses_exact_enum_and_lossless_quantity_names() {
        let summary = DiscoveryWindowSummary {
            window_revision: 2,
            ruleset_revision: 4,
            opened_unix_ms: 100,
            closed_unix_ms: 200,
            evaluated_unix_ms: 205,
            decision: QualificationDecision::Unknown,
            completeness: WindowCompleteness::Incomplete,
            reason_codes: vec!["WINDOW_INTERRUPTED".to_owned()],
            trades: 8,
            buys: 6,
            sells: 2,
            unique_traders: 5,
            unique_buyers: 4,
            unique_sellers: 2,
            buy_base_volume_units: "10000000000000000".to_owned(),
            sell_base_volume_units: "3000000000000000".to_owned(),
            buy_quote_volume_units: "250000000".to_owned(),
            sell_quote_volume_units: "100000000".to_owned(),
            maximum_single_wallet_quote_share_bps: Some(7_500),
            price_change_bps: Some(-125),
            first_base_reserve_units: Some("9000000000000000000".to_owned()),
            first_quote_reserve_units: Some("30000000000".to_owned()),
            latest_base_reserve_units: Some("8500000000000000000".to_owned()),
            latest_quote_reserve_units: Some("34000000000".to_owned()),
        };
        let encoded = serde_json::to_value(summary).expect("contract should serialize");

        assert_eq!(encoded["decision"], "UNKNOWN");
        assert_eq!(encoded["completeness"], "INCOMPLETE");
        assert_eq!(encoded["buy_base_volume_units"], "10000000000000000");
        assert_eq!(encoded["first_base_reserve_units"], "9000000000000000000");
        assert_eq!(encoded["price_change_bps"], -125);
    }

    #[test]
    fn qualification_discovery_enums_are_stable() {
        assert_eq!(
            serde_json::to_value(DiscoveryMode::QualifiedOnly).expect("mode should serialize"),
            "QUALIFIED_ONLY"
        );
        assert_eq!(
            serde_json::to_value(DiscoveryStage::Qualified).expect("stage should serialize"),
            "QUALIFIED"
        );
        assert_eq!(
            serde_json::to_value(QualificationDecision::Pass).expect("decision should serialize"),
            "PASS"
        );
        assert_eq!(
            serde_json::to_value(QualificationDecision::Reject).expect("decision should serialize"),
            "REJECT"
        );
        assert_eq!(
            serde_json::to_value(WindowCompleteness::Complete)
                .expect("completeness should serialize"),
            "COMPLETE"
        );
    }

    #[test]
    fn prequalification_token_json_remains_readable_after_upgrade() {
        let token: DiscoveryToken = serde_json::from_value(serde_json::json!({
            "mint": "mint",
            "name": null,
            "symbol": null,
            "primary_venue": "PUMP_BONDING_CURVE",
            "market_address": "curve",
            "quote_mint": null,
            "source_program": "PUMP",
            "stage": "OBSERVED",
            "last_event_kind": "CREATE",
            "observed_slot": 1,
            "first_observed_unix_ms": 1,
            "last_observed_unix_ms": 1,
            "latest_signature": "signature",
            "activity": {
                "trades": 0,
                "buys": 0,
                "sells": 0,
                "unique_traders": 0,
                "base_volume_units": "0",
                "quote_volume_units": "0"
            },
            "risk_score": null,
            "opportunity_score": null
        }))
        .expect("the optional qualification field must be backward compatible");

        assert_eq!(token.qualification, None);
    }
}
