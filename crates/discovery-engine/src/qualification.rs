use serde::Serialize;
use soldisco_domain::{AssessmentDecision, Network, RuleResult};
use thiserror::Error;

use crate::{BASIS_POINTS_SCALE, MarketWindowSnapshot, SnapshotError};

pub const NATIVE_SOL_MINT: &str = "11111111111111111111111111111111";
pub const WRAPPED_SOL_MINT: &str = "So11111111111111111111111111111111111111112";
pub const CANONICAL_USDC_MINT: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

const RULE_VERSION: &str = "1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuoteAssetClass {
    Native,
    Stable,
    Unsupported,
}

#[must_use]
pub fn classify_quote_asset(snapshot: &MarketWindowSnapshot) -> QuoteAssetClass {
    match snapshot.market.quote_mint.as_deref() {
        Some(NATIVE_SOL_MINT | WRAPPED_SOL_MINT) => QuoteAssetClass::Native,
        Some(CANONICAL_USDC_MINT) if snapshot.market.network == Network::SolanaMainnet => {
            QuoteAssetClass::Stable
        }
        _ => QuoteAssetClass::Unsupported,
    }
}

/// Global, strategy-neutral admission policy. These thresholds qualify the
/// observed activity for deeper checks; they do not assert that a token is
/// safe or that low activity is a scam.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QualificationPolicy {
    pub ruleset_version: String,
    pub minimum_trades: u64,
    pub minimum_unique_traders: u64,
    pub minimum_buys: u64,
    pub minimum_sells: u64,
    pub minimum_native_quote_volume_units: u128,
    pub minimum_stable_quote_volume_units: u128,
    pub maximum_single_wallet_quote_volume_bps: u16,
}

impl Default for QualificationPolicy {
    fn default() -> Self {
        Self {
            ruleset_version: "initial-qualification-v1".to_owned(),
            minimum_trades: 5,
            minimum_unique_traders: 3,
            minimum_buys: 1,
            minimum_sells: 1,
            minimum_native_quote_volume_units: 50_000_000,
            minimum_stable_quote_volume_units: 5_000_000,
            maximum_single_wallet_quote_volume_bps: 9_000,
        }
    }
}

impl QualificationPolicy {
    pub fn validate(&self) -> Result<(), QualificationPolicyError> {
        if self.ruleset_version.trim().is_empty() {
            return Err(QualificationPolicyError::MissingRulesetVersion);
        }
        if self.maximum_single_wallet_quote_volume_bps > BASIS_POINTS_SCALE {
            return Err(QualificationPolicyError::InvalidMaximumWalletShare {
                actual_bps: self.maximum_single_wallet_quote_volume_bps,
            });
        }
        if self.minimum_unique_traders > self.minimum_trades {
            return Err(QualificationPolicyError::MinimumUniqueTradersExceedTrades);
        }
        if self.minimum_buys > self.minimum_trades {
            return Err(QualificationPolicyError::MinimumBuysExceedTrades);
        }
        if self.minimum_sells > self.minimum_trades {
            return Err(QualificationPolicyError::MinimumSellsExceedTrades);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum QualificationRuleId {
    CompleteWindow,
    SupportedQuoteAsset,
    MinimumTrades,
    MinimumUniqueTraders,
    MinimumBuys,
    MinimumSells,
    MinimumQuoteVolume,
    MaximumWalletConcentration,
}

impl QualificationRuleId {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CompleteWindow => "qualification.window-complete",
            Self::SupportedQuoteAsset => "qualification.supported-quote-asset",
            Self::MinimumTrades => "qualification.minimum-trades",
            Self::MinimumUniqueTraders => "qualification.minimum-unique-traders",
            Self::MinimumBuys => "qualification.minimum-buys",
            Self::MinimumSells => "qualification.minimum-sells",
            Self::MinimumQuoteVolume => "qualification.minimum-quote-volume",
            Self::MaximumWalletConcentration => "qualification.maximum-single-wallet-quote-volume",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct QualificationAssessment {
    pub ruleset_version: String,
    pub decision: AssessmentDecision,
    pub rules: Vec<RuleResult>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum QualificationPolicyError {
    #[error("qualification ruleset version must not be empty")]
    MissingRulesetVersion,
    #[error(
        "maximum single-wallet quote-volume share must be at most {BASIS_POINTS_SCALE} bps, got {actual_bps}"
    )]
    InvalidMaximumWalletShare { actual_bps: u16 },
    #[error("minimum unique traders cannot exceed minimum trades")]
    MinimumUniqueTradersExceedTrades,
    #[error("minimum buys cannot exceed minimum trades")]
    MinimumBuysExceedTrades,
    #[error("minimum sells cannot exceed minimum trades")]
    MinimumSellsExceedTrades,
    #[error(transparent)]
    Snapshot(#[from] SnapshotError),
}

/// Evaluates one completed snapshot with explicit, versioned evidence.
///
/// Incomplete collection produces only `UNKNOWN` rules. A rejection represents
/// failed qualification (activity quality), not a declaration that the token
/// is a scam.
pub fn evaluate_qualification(
    snapshot: &MarketWindowSnapshot,
    policy: &QualificationPolicy,
) -> Result<QualificationAssessment, QualificationPolicyError> {
    policy.validate()?;
    if !snapshot.completeness.is_complete() {
        let reason = snapshot
            .completeness
            .reason_code()
            .unwrap_or("WINDOW_INCOMPLETE");
        let rules = all_rule_ids()
            .into_iter()
            .map(|rule_id| {
                rule(
                    rule_id,
                    AssessmentDecision::Unknown,
                    reason,
                    Some(window_evidence(snapshot)),
                    snapshot.latest_observed_slot,
                )
            })
            .collect();
        return Ok(QualificationAssessment {
            ruleset_version: policy.ruleset_version.clone(),
            decision: AssessmentDecision::Unknown,
            rules,
        });
    }

    let quote_asset = classify_quote_asset(snapshot);
    let total_quote_volume = snapshot.metrics.total_quote_volume_units()?;
    let mut rules = Vec::with_capacity(all_rule_ids().len());
    rules.push(rule(
        QualificationRuleId::CompleteWindow,
        AssessmentDecision::Pass,
        "WINDOW_COMPLETE",
        Some(window_evidence(snapshot)),
        snapshot.latest_observed_slot,
    ));
    rules.push(match quote_asset {
        QuoteAssetClass::Native => rule(
            QualificationRuleId::SupportedQuoteAsset,
            AssessmentDecision::Pass,
            "SUPPORTED_NATIVE_QUOTE_ASSET",
            snapshot.market.quote_mint.clone(),
            snapshot.latest_observed_slot,
        ),
        QuoteAssetClass::Stable => rule(
            QualificationRuleId::SupportedQuoteAsset,
            AssessmentDecision::Pass,
            "SUPPORTED_STABLE_QUOTE_ASSET",
            snapshot.market.quote_mint.clone(),
            snapshot.latest_observed_slot,
        ),
        QuoteAssetClass::Unsupported => rule(
            QualificationRuleId::SupportedQuoteAsset,
            AssessmentDecision::Reject,
            "UNSUPPORTED_QUOTE_ASSET",
            Some(
                snapshot
                    .market
                    .quote_mint
                    .clone()
                    .unwrap_or_else(|| "NONE".to_owned()),
            ),
            snapshot.latest_observed_slot,
        ),
    });
    rules.push(minimum_rule(
        QualificationRuleId::MinimumTrades,
        snapshot.metrics.trades,
        policy.minimum_trades,
        snapshot.latest_observed_slot,
    ));
    rules.push(minimum_rule(
        QualificationRuleId::MinimumUniqueTraders,
        snapshot.metrics.unique_traders,
        policy.minimum_unique_traders,
        snapshot.latest_observed_slot,
    ));
    rules.push(minimum_rule(
        QualificationRuleId::MinimumBuys,
        snapshot.metrics.buys,
        policy.minimum_buys,
        snapshot.latest_observed_slot,
    ));
    rules.push(minimum_rule(
        QualificationRuleId::MinimumSells,
        snapshot.metrics.sells,
        policy.minimum_sells,
        snapshot.latest_observed_slot,
    ));

    rules.push(match quote_asset {
        QuoteAssetClass::Native => minimum_rule(
            QualificationRuleId::MinimumQuoteVolume,
            total_quote_volume,
            policy.minimum_native_quote_volume_units,
            snapshot.latest_observed_slot,
        ),
        QuoteAssetClass::Stable => minimum_rule(
            QualificationRuleId::MinimumQuoteVolume,
            total_quote_volume,
            policy.minimum_stable_quote_volume_units,
            snapshot.latest_observed_slot,
        ),
        QuoteAssetClass::Unsupported => rule(
            QualificationRuleId::MinimumQuoteVolume,
            AssessmentDecision::Unknown,
            "QUOTE_VOLUME_THRESHOLD_UNAVAILABLE",
            Some(format!("observed_units={total_quote_volume}")),
            snapshot.latest_observed_slot,
        ),
    });

    rules.push(match &snapshot.metrics.largest_wallet_quote_volume {
        Some(largest) => {
            let passes = largest.share_bps <= policy.maximum_single_wallet_quote_volume_bps;
            rule(
                QualificationRuleId::MaximumWalletConcentration,
                if passes {
                    AssessmentDecision::Pass
                } else {
                    AssessmentDecision::Reject
                },
                if passes {
                    "WALLET_CONCENTRATION_WITHIN_LIMIT"
                } else {
                    "WALLET_CONCENTRATION_EXCEEDS_LIMIT"
                },
                Some(format!(
                    "wallet={};actual_bps={};maximum_bps={}",
                    largest.wallet,
                    largest.share_bps,
                    policy.maximum_single_wallet_quote_volume_bps
                )),
                snapshot.latest_observed_slot,
            )
        }
        None => rule(
            QualificationRuleId::MaximumWalletConcentration,
            AssessmentDecision::Unknown,
            "WALLET_CONCENTRATION_UNAVAILABLE",
            Some(format!("observed_quote_units={total_quote_volume}")),
            snapshot.latest_observed_slot,
        ),
    });

    let decision = aggregate(&rules);
    Ok(QualificationAssessment {
        ruleset_version: policy.ruleset_version.clone(),
        decision,
        rules,
    })
}

fn all_rule_ids() -> [QualificationRuleId; 8] {
    [
        QualificationRuleId::CompleteWindow,
        QualificationRuleId::SupportedQuoteAsset,
        QualificationRuleId::MinimumTrades,
        QualificationRuleId::MinimumUniqueTraders,
        QualificationRuleId::MinimumBuys,
        QualificationRuleId::MinimumSells,
        QualificationRuleId::MinimumQuoteVolume,
        QualificationRuleId::MaximumWalletConcentration,
    ]
}

fn minimum_rule<T>(
    rule_id: QualificationRuleId,
    actual: T,
    minimum: T,
    observed_slot: Option<u64>,
) -> RuleResult
where
    T: Copy + Ord + std::fmt::Display,
{
    let passes = actual >= minimum;
    rule(
        rule_id,
        if passes {
            AssessmentDecision::Pass
        } else {
            AssessmentDecision::Reject
        },
        minimum_reason_code(rule_id, passes),
        Some(format!("actual={actual};minimum={minimum}")),
        observed_slot,
    )
}

const fn minimum_reason_code(rule_id: QualificationRuleId, passes: bool) -> &'static str {
    match (rule_id, passes) {
        (QualificationRuleId::MinimumTrades, true) => "MINIMUM_TRADES_MET",
        (QualificationRuleId::MinimumTrades, false) => "TRADES_BELOW_MINIMUM",
        (QualificationRuleId::MinimumUniqueTraders, true) => "MINIMUM_UNIQUE_TRADERS_MET",
        (QualificationRuleId::MinimumUniqueTraders, false) => "UNIQUE_TRADERS_BELOW_MINIMUM",
        (QualificationRuleId::MinimumBuys, true) => "MINIMUM_BUYS_MET",
        (QualificationRuleId::MinimumBuys, false) => "BUYS_BELOW_MINIMUM",
        (QualificationRuleId::MinimumSells, true) => "MINIMUM_SELLS_MET",
        (QualificationRuleId::MinimumSells, false) => "SELLS_BELOW_MINIMUM",
        (QualificationRuleId::MinimumQuoteVolume, true) => "MINIMUM_QUOTE_VOLUME_MET",
        (QualificationRuleId::MinimumQuoteVolume, false) => "QUOTE_VOLUME_BELOW_MINIMUM",
        _ => {
            if passes {
                "MINIMUM_MET"
            } else {
                "BELOW_MINIMUM"
            }
        }
    }
}

fn rule(
    rule_id: QualificationRuleId,
    decision: AssessmentDecision,
    reason_code: impl Into<String>,
    evidence_reference: Option<String>,
    observed_slot: Option<u64>,
) -> RuleResult {
    RuleResult {
        rule_id: rule_id.as_str().to_owned(),
        rule_version: RULE_VERSION.to_owned(),
        decision,
        reason_code: reason_code.into(),
        evidence_reference,
        observed_slot,
    }
}

fn aggregate(rules: &[RuleResult]) -> AssessmentDecision {
    if rules
        .iter()
        .any(|result| result.decision == AssessmentDecision::Reject)
    {
        AssessmentDecision::Reject
    } else if rules
        .iter()
        .any(|result| result.decision == AssessmentDecision::Unknown)
    {
        AssessmentDecision::Unknown
    } else {
        AssessmentDecision::Pass
    }
}

fn window_evidence(snapshot: &MarketWindowSnapshot) -> String {
    format!(
        "market={};opened_at_unix_ms={};closes_at_unix_ms={};observations={}",
        snapshot.market.market_address,
        snapshot.opened_at_unix_ms,
        snapshot.closes_at_unix_ms,
        snapshot.metrics.observations
    )
}

#[cfg(test)]
mod tests {
    use soldisco_domain::{
        ChainCoordinate, Commitment, MarketIdentity, Network, NormalizedObservation,
        ObservationKey, ObservationPayload, SourceProgram, TradeSide, Venue,
    };

    use super::*;
    use crate::{SnapshotCompleteness, build_market_window_snapshot};

    fn market(quote_mint: Option<&str>, network: Network) -> MarketIdentity {
        MarketIdentity {
            network,
            mint: "mint".to_owned(),
            venue: Venue::PumpBondingCurve,
            market_address: "curve".to_owned(),
            quote_mint: quote_mint.map(str::to_owned),
        }
    }

    fn trade(
        market: &MarketIdentity,
        event_index: u16,
        side: TradeSide,
        wallet: &str,
        quote_amount_units: u64,
    ) -> NormalizedObservation {
        NormalizedObservation {
            key: ObservationKey {
                network: market.network,
                program: SourceProgram::Pump,
                coordinate: ChainCoordinate {
                    slot: 100 + u64::from(event_index),
                    transaction_index: Some(0),
                    signature: format!("signature-{event_index}"),
                    instruction_index: 0,
                    event_index,
                },
            },
            commitment: Commitment::Confirmed,
            schema_version: 1,
            decoder_version: "test".to_owned(),
            event_kind: "TRADE".to_owned(),
            market: market.clone(),
            source_event_time_unix_ms: None,
            received_time_unix_ms: 100 + i64::from(event_index),
            raw_evidence_hash: format!("hash-{event_index}"),
            source_evidence_base64: String::new(),
            source_details: Default::default(),
            payload: ObservationPayload::Trade {
                side,
                wallet: wallet.to_owned(),
                base_amount_units: 1,
                quote_amount_units,
                base_reserve_units: None,
                quote_reserve_units: None,
            },
        }
    }

    fn passing_snapshot(quote_mint: Option<&str>) -> MarketWindowSnapshot {
        let market = market(quote_mint, Network::SolanaMainnet);
        let observations = vec![
            trade(&market, 0, TradeSide::Buy, "a", 40),
            trade(&market, 1, TradeSide::Buy, "b", 20),
            trade(&market, 2, TradeSide::Sell, "c", 40),
        ];
        build_market_window_snapshot(
            &market,
            100,
            200,
            SnapshotCompleteness::Complete,
            &observations,
        )
        .expect("snapshot")
    }

    fn policy() -> QualificationPolicy {
        QualificationPolicy {
            ruleset_version: "test-v1".to_owned(),
            minimum_trades: 3,
            minimum_unique_traders: 3,
            minimum_buys: 2,
            minimum_sells: 1,
            minimum_native_quote_volume_units: 100,
            minimum_stable_quote_volume_units: 100,
            maximum_single_wallet_quote_volume_bps: 4_000,
        }
    }

    #[test]
    fn inclusive_threshold_boundaries_pass() {
        let assessment =
            evaluate_qualification(&passing_snapshot(Some(WRAPPED_SOL_MINT)), &policy())
                .expect("assessment");

        assert_eq!(assessment.decision, AssessmentDecision::Pass);
        assert!(
            assessment
                .rules
                .iter()
                .all(|rule| rule.decision == AssessmentDecision::Pass)
        );
    }

    #[test]
    fn native_system_mint_uses_native_threshold() {
        let mut policy = policy();
        policy.minimum_native_quote_volume_units = 100;
        policy.minimum_stable_quote_volume_units = 101;

        let assessment = evaluate_qualification(&passing_snapshot(Some(NATIVE_SOL_MINT)), &policy)
            .expect("assessment");

        assert_eq!(assessment.decision, AssessmentDecision::Pass);
    }

    #[test]
    fn canonical_mainnet_usdc_uses_stable_threshold() {
        let mut policy = policy();
        policy.minimum_native_quote_volume_units = 101;
        policy.minimum_stable_quote_volume_units = 100;

        let assessment =
            evaluate_qualification(&passing_snapshot(Some(CANONICAL_USDC_MINT)), &policy)
                .expect("assessment");

        assert_eq!(assessment.decision, AssessmentDecision::Pass);
    }

    #[test]
    fn unsupported_quote_is_an_explicit_reject() {
        let assessment =
            evaluate_qualification(&passing_snapshot(Some("unknown-quote")), &policy())
                .expect("assessment");
        let quote_rule = assessment
            .rules
            .iter()
            .find(|rule| rule.rule_id == QualificationRuleId::SupportedQuoteAsset.as_str())
            .expect("quote rule");

        assert_eq!(assessment.decision, AssessmentDecision::Reject);
        assert_eq!(quote_rule.decision, AssessmentDecision::Reject);
        assert_eq!(quote_rule.reason_code, "UNSUPPORTED_QUOTE_ASSET");
    }

    #[test]
    fn canonical_mainnet_usdc_address_is_not_assumed_canonical_on_devnet() {
        let market = market(Some(CANONICAL_USDC_MINT), Network::SolanaDevnet);
        let observations = vec![
            trade(&market, 0, TradeSide::Buy, "a", 40),
            trade(&market, 1, TradeSide::Buy, "b", 20),
            trade(&market, 2, TradeSide::Sell, "c", 40),
        ];
        let snapshot = build_market_window_snapshot(
            &market,
            100,
            200,
            SnapshotCompleteness::Complete,
            &observations,
        )
        .expect("snapshot");

        assert_eq!(
            classify_quote_asset(&snapshot),
            QuoteAssetClass::Unsupported
        );
    }

    #[test]
    fn incomplete_window_produces_unknown_rules_only() {
        let complete = passing_snapshot(Some(WRAPPED_SOL_MINT));
        let snapshot = MarketWindowSnapshot {
            completeness: SnapshotCompleteness::Incomplete {
                reason_code: "COLLECTOR_GAP".to_owned(),
            },
            ..complete
        };

        let assessment = evaluate_qualification(&snapshot, &policy()).expect("assessment");

        assert_eq!(assessment.decision, AssessmentDecision::Unknown);
        assert!(
            assessment
                .rules
                .iter()
                .all(|rule| rule.decision == AssessmentDecision::Unknown)
        );
        assert!(
            assessment
                .rules
                .iter()
                .all(|rule| rule.reason_code == "COLLECTOR_GAP")
        );
    }

    #[test]
    fn missing_cpi_instruction_context_cannot_produce_a_pass() {
        let complete = passing_snapshot(Some(WRAPPED_SOL_MINT));
        let snapshot = MarketWindowSnapshot {
            completeness: SnapshotCompleteness::Incomplete {
                reason_code: "CPI_INSTRUCTION_DATA_UNAVAILABLE".to_owned(),
            },
            ..complete
        };

        let assessment = evaluate_qualification(&snapshot, &policy()).expect("assessment");

        assert_eq!(assessment.decision, AssessmentDecision::Unknown);
        assert_ne!(assessment.decision, AssessmentDecision::Pass);
        assert!(assessment.rules.iter().all(|rule| {
            rule.decision == AssessmentDecision::Unknown
                && rule.reason_code == "CPI_INSTRUCTION_DATA_UNAVAILABLE"
        }));
    }

    #[test]
    fn wallet_concentration_above_boundary_rejects() {
        let mut policy = policy();
        policy.maximum_single_wallet_quote_volume_bps = 3_999;
        let assessment = evaluate_qualification(&passing_snapshot(Some(WRAPPED_SOL_MINT)), &policy)
            .expect("assessment");
        let concentration = assessment
            .rules
            .iter()
            .find(|rule| rule.rule_id == QualificationRuleId::MaximumWalletConcentration.as_str())
            .expect("concentration");

        assert_eq!(concentration.decision, AssessmentDecision::Reject);
        assert_eq!(
            concentration.reason_code,
            "WALLET_CONCENTRATION_EXCEEDS_LIMIT"
        );
    }

    #[test]
    fn fractional_wallet_concentration_cannot_round_down_into_a_pass() {
        let market = market(Some(WRAPPED_SOL_MINT), Network::SolanaMainnet);
        let snapshot = build_market_window_snapshot(
            &market,
            100,
            200,
            SnapshotCompleteness::Complete,
            &[
                trade(&market, 0, TradeSide::Buy, "dominant", 9_001),
                trade(&market, 1, TradeSide::Sell, "other", 1_000),
            ],
        )
        .expect("snapshot");
        let policy = QualificationPolicy {
            ruleset_version: "exact-concentration-v1".to_owned(),
            minimum_trades: 2,
            minimum_unique_traders: 2,
            minimum_buys: 1,
            minimum_sells: 1,
            minimum_native_quote_volume_units: 1,
            minimum_stable_quote_volume_units: 1,
            maximum_single_wallet_quote_volume_bps: 9_000,
        };

        assert_eq!(
            snapshot
                .metrics
                .largest_wallet_quote_volume
                .as_ref()
                .expect("largest wallet")
                .share_bps,
            9_001
        );
        let assessment = evaluate_qualification(&snapshot, &policy).expect("assessment");
        let concentration = assessment
            .rules
            .iter()
            .find(|rule| rule.rule_id == QualificationRuleId::MaximumWalletConcentration.as_str())
            .expect("concentration rule");
        assert_eq!(concentration.decision, AssessmentDecision::Reject);
    }

    #[test]
    fn invalid_policy_is_rejected_before_evaluation() {
        let mut policy = policy();
        policy.maximum_single_wallet_quote_volume_bps = 10_001;

        assert_eq!(
            evaluate_qualification(&passing_snapshot(Some(WRAPPED_SOL_MINT)), &policy),
            Err(QualificationPolicyError::InvalidMaximumWalletShare { actual_bps: 10_001 })
        );
    }

    #[test]
    fn low_activity_is_labeled_as_below_minimum_not_as_a_scam() {
        let mut policy = policy();
        policy.minimum_trades = 4;
        let assessment = evaluate_qualification(&passing_snapshot(Some(WRAPPED_SOL_MINT)), &policy)
            .expect("assessment");

        assert_eq!(assessment.decision, AssessmentDecision::Reject);
        assert!(
            assessment
                .rules
                .iter()
                .any(|rule| rule.reason_code == "TRADES_BELOW_MINIMUM")
        );
        assert!(
            assessment
                .rules
                .iter()
                .all(|rule| !rule.reason_code.contains("SCAM"))
        );
    }
}
