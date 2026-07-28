use std::collections::BTreeMap;

use soldisco_api_contracts::{
    ApprovedToken, DiscoverySnapshot, RejectionSummary, ScreeningCounters,
};

#[derive(Clone, Debug, Default)]
pub struct DiscoveryProjection {
    sequence: u64,
    counters: ScreeningCounters,
    approved: BTreeMap<String, ApprovedToken>,
    rejection_reasons: BTreeMap<String, RejectionSummary>,
}

impl DiscoveryProjection {
    pub fn mark_pending(&mut self) {
        self.counters.pending = self.counters.pending.saturating_add(1);
        self.advance();
    }

    pub fn approve(&mut self, token: ApprovedToken) {
        self.counters.pending = self.counters.pending.saturating_sub(1);
        if self.approved.insert(token.mint.clone(), token).is_none() {
            self.counters.approved = self.counters.approved.saturating_add(1);
        }
        self.advance();
    }

    pub fn reject(&mut self, reason_code: impl Into<String>, seen_at_unix_ms: i64) {
        self.counters.pending = self.counters.pending.saturating_sub(1);
        self.counters.rejected = self.counters.rejected.saturating_add(1);
        let reason_code = reason_code.into();
        let summary =
            self.rejection_reasons
                .entry(reason_code.clone())
                .or_insert(RejectionSummary {
                    reason_code,
                    count: 0,
                    last_seen_unix_ms: seen_at_unix_ms,
                });
        summary.count = summary.count.saturating_add(1);
        summary.last_seen_unix_ms = seen_at_unix_ms;
        self.advance();
    }

    #[must_use]
    pub fn snapshot(&self) -> DiscoverySnapshot {
        DiscoverySnapshot {
            sequence: self.sequence,
            approved_tokens: self.approved.values().cloned().collect(),
            counters: self.counters.clone(),
            rejection_reasons: self.rejection_reasons.values().cloned().collect(),
        }
    }

    fn advance(&mut self) {
        self.sequence = self.sequence.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use soldisco_api_contracts::ApprovedToken;
    use soldisco_domain::{AssessmentDecision, Score, Venue};

    use super::DiscoveryProjection;

    #[test]
    fn only_approved_tokens_enter_the_feed() {
        let mut projection = DiscoveryProjection::default();
        projection.mark_pending();
        projection.reject("HOLDER_CONCENTRATION", 10);
        projection.mark_pending();
        projection.approve(ApprovedToken {
            mint: "approved-mint".to_owned(),
            symbol: None,
            primary_venue: Venue::PumpBondingCurve,
            deterministic_decision: AssessmentDecision::Pass,
            risk_score: Score::new(20).expect("valid score"),
            opportunity_score: Score::new(75).expect("valid score"),
            observed_slot: 1,
        });

        let snapshot = projection.snapshot();
        assert_eq!(snapshot.approved_tokens.len(), 1);
        assert_eq!(snapshot.counters.rejected, 1);
        assert_eq!(snapshot.rejection_reasons[0].count, 1);
    }
}
