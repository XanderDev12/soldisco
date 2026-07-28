use serde::{Deserialize, Serialize};

/// Authoritative deterministic outcome. Missing required evidence is `Unknown`,
/// never an implicit pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AssessmentDecision {
    Pass,
    Reject,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuleResult {
    pub rule_id: String,
    pub rule_version: String,
    pub decision: AssessmentDecision,
    pub reason_code: String,
    pub evidence_reference: Option<String>,
    pub observed_slot: Option<u64>,
}

impl RuleResult {
    #[must_use]
    pub fn unknown(
        rule_id: impl Into<String>,
        rule_version: impl Into<String>,
        reason_code: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            rule_version: rule_version.into(),
            decision: AssessmentDecision::Unknown,
            reason_code: reason_code.into(),
            evidence_reference: None,
            observed_slot: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AssessmentDecision, RuleResult};

    #[test]
    fn missing_evidence_is_explicitly_unknown() {
        let result = RuleResult::unknown("mint-authority", "1", "EVIDENCE_MISSING");

        assert_eq!(result.decision, AssessmentDecision::Unknown);
        assert_eq!(result.rule_version, "1");
        assert_eq!(result.reason_code, "EVIDENCE_MISSING");
    }
}
