use serde::{Deserialize, Serialize};
use soldisco_domain::{AssessmentDecision, RuleResult, Score};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EvidenceAvailability {
    Available,
    Missing,
    Stale,
    Invalid,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RiskAssessment {
    pub decision: AssessmentDecision,
    pub risk_score: Option<Score>,
    pub opportunity_score: Option<Score>,
    pub rules: Vec<RuleResult>,
}

impl RiskAssessment {
    #[must_use]
    pub fn from_rules(
        rules: Vec<RuleResult>,
        risk_score: Option<Score>,
        opportunity_score: Option<Score>,
    ) -> Self {
        let decision = if rules
            .iter()
            .any(|rule| rule.decision == AssessmentDecision::Reject)
        {
            AssessmentDecision::Reject
        } else if rules
            .iter()
            .any(|rule| rule.decision == AssessmentDecision::Unknown)
            || rules.is_empty()
        {
            AssessmentDecision::Unknown
        } else {
            AssessmentDecision::Pass
        };

        Self {
            decision,
            risk_score,
            opportunity_score,
            rules,
        }
    }
}

#[must_use]
pub fn required_evidence_rule(
    rule_id: impl Into<String>,
    availability: EvidenceAvailability,
) -> RuleResult {
    let (decision, reason_code) = match availability {
        EvidenceAvailability::Available => (AssessmentDecision::Pass, "EVIDENCE_AVAILABLE"),
        EvidenceAvailability::Missing => (AssessmentDecision::Unknown, "EVIDENCE_MISSING"),
        EvidenceAvailability::Stale => (AssessmentDecision::Unknown, "EVIDENCE_STALE"),
        EvidenceAvailability::Invalid => (AssessmentDecision::Reject, "EVIDENCE_INVALID"),
    };

    RuleResult {
        rule_id: rule_id.into(),
        rule_version: "1".to_owned(),
        decision,
        reason_code: reason_code.to_owned(),
        evidence_reference: None,
        observed_slot: None,
    }
}

#[cfg(test)]
mod tests {
    use soldisco_domain::{AssessmentDecision, Score};

    use super::{EvidenceAvailability, RiskAssessment, required_evidence_rule};

    #[test]
    fn missing_required_evidence_fails_closed() {
        let assessment = RiskAssessment::from_rules(
            vec![required_evidence_rule(
                "pool-state",
                EvidenceAvailability::Missing,
            )],
            None,
            None,
        );

        assert_eq!(assessment.decision, AssessmentDecision::Unknown);
    }

    #[test]
    fn risk_and_opportunity_scores_remain_separate() {
        let assessment = RiskAssessment::from_rules(
            vec![required_evidence_rule(
                "pool-state",
                EvidenceAvailability::Available,
            )],
            Some(Score::new(80).expect("valid score")),
            Some(Score::new(95).expect("valid score")),
        );

        assert_eq!(
            assessment.risk_score,
            Some(Score::new(80).expect("valid score"))
        );
        assert_eq!(
            assessment.opportunity_score,
            Some(Score::new(95).expect("valid score"))
        );
    }
}
