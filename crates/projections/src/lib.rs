use std::collections::BTreeMap;

use soldisco_api_contracts::{
    DiscoveryCounters, DiscoveryMode, DiscoverySnapshot, DiscoveryStage, DiscoveryToken,
    RejectionSummary,
};
use soldisco_domain::{SourceProgram, Venue};
use thiserror::Error;

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProjectionError {
    #[error("{field} must not be empty")]
    EmptyField { field: &'static str },
    #[error("{field} must contain only unsigned decimal digits")]
    InvalidUnsignedDecimal { field: &'static str },
    #[error("discovery activity buys plus sells must equal trades")]
    InvalidTradeTotals,
    #[error("discovery activity unique traders cannot exceed trades")]
    InvalidUniqueTraderTotal,
    #[error("first observed time cannot be later than last observed time")]
    InvalidObservationTimes,
    #[error("source program {source_program:?} is incompatible with venue {venue:?}")]
    SourceVenueMismatch {
        source_program: SourceProgram,
        venue: Venue,
    },
    #[error("observe requires the OBSERVED stage")]
    ObserveRequiresObservedStage,
    #[error("approve requires the APPROVED stage")]
    ApproveRequiresApprovedStage,
    #[error("token {mint} must be observed before it can be approved")]
    CandidateNotObserved { mint: String },
    #[error("token {mint} approval does not match its observed market identity")]
    ApprovalMarketMismatch { mint: String },
    #[error("snapshot approved counter does not match its approved tokens")]
    ApprovedCounterMismatch,
    #[error("a truncated browser snapshot cannot restore the complete projection")]
    TruncatedSnapshot,
    #[error("APPROVED_ONLY snapshots cannot contain observed-stage tokens")]
    ObservedTokenInApprovedOnlySnapshot,
    #[error("rejection reason code must not be empty")]
    EmptyRejectionReason,
    #[error("rejection summaries must have a non-zero count")]
    EmptyRejectionSummary,
    #[error("snapshot contains duplicate token mint {mint}")]
    DuplicateMint { mint: String },
    #[error("snapshot contains duplicate rejection reason {reason_code}")]
    DuplicateRejectionReason { reason_code: String },
}

#[derive(Clone, Debug)]
pub struct DiscoveryProjection {
    sequence: u64,
    mode: DiscoveryMode,
    counters: DiscoveryCounters,
    tokens: BTreeMap<String, DiscoveryToken>,
    rejection_reasons: BTreeMap<String, RejectionSummary>,
}

impl Default for DiscoveryProjection {
    fn default() -> Self {
        Self {
            sequence: 0,
            mode: DiscoveryMode::ObserveAll,
            counters: DiscoveryCounters::default(),
            tokens: BTreeMap::new(),
            rejection_reasons: BTreeMap::new(),
        }
    }
}

impl DiscoveryProjection {
    pub fn try_from_snapshot(snapshot: DiscoverySnapshot) -> Result<Self, ProjectionError> {
        if snapshot.tokens_truncated {
            return Err(ProjectionError::TruncatedSnapshot);
        }
        let mut tokens = BTreeMap::new();
        let mut approved_count = 0_u64;

        for token in snapshot.tokens {
            validate_token(&token)?;
            if snapshot.mode == DiscoveryMode::ApprovedOnly
                && token.stage != DiscoveryStage::Approved
            {
                return Err(ProjectionError::ObservedTokenInApprovedOnlySnapshot);
            }
            if token.stage == DiscoveryStage::Approved {
                approved_count = approved_count.saturating_add(1);
            }
            let mint = token.mint.clone();
            if tokens.insert(mint.clone(), token).is_some() {
                return Err(ProjectionError::DuplicateMint { mint });
            }
        }

        if approved_count != snapshot.counters.approved {
            return Err(ProjectionError::ApprovedCounterMismatch);
        }

        let mut rejection_reasons = BTreeMap::new();
        for summary in snapshot.rejection_reasons {
            validate_non_empty(&summary.reason_code, "reason_code")
                .map_err(|_| ProjectionError::EmptyRejectionReason)?;
            if summary.count == 0 {
                return Err(ProjectionError::EmptyRejectionSummary);
            }
            let reason_code = summary.reason_code.clone();
            if rejection_reasons
                .insert(reason_code.clone(), summary)
                .is_some()
            {
                return Err(ProjectionError::DuplicateRejectionReason { reason_code });
            }
        }

        Ok(Self {
            sequence: snapshot.sequence,
            mode: snapshot.mode,
            counters: snapshot.counters,
            tokens,
            rejection_reasons,
        })
    }

    pub fn mark_pending(&mut self) {
        self.counters.pending = self.counters.pending.saturating_add(1);
        self.advance();
    }

    /// Records a structurally valid candidate without treating it as approved.
    pub fn observe(&mut self, mut token: DiscoveryToken) -> Result<(), ProjectionError> {
        validate_token(&token)?;
        if token.stage != DiscoveryStage::Observed {
            return Err(ProjectionError::ObserveRequiresObservedStage);
        }

        self.counters.pending = self.counters.pending.saturating_sub(1);
        match self.tokens.get(&token.mint) {
            Some(existing) if existing.observed_slot > token.observed_slot => {}
            Some(existing) => {
                let was_approved = existing.stage == DiscoveryStage::Approved;
                if was_approved && same_market(existing, &token) {
                    token.stage = DiscoveryStage::Approved;
                    token.risk_score = token.risk_score.or(existing.risk_score);
                    token.opportunity_score =
                        token.opportunity_score.or(existing.opportunity_score);
                }
                if was_approved && token.stage != DiscoveryStage::Approved {
                    self.counters.approved = self.counters.approved.saturating_sub(1);
                }
                self.tokens.insert(token.mint.clone(), token);
            }
            None => {
                self.counters.observed = self.counters.observed.saturating_add(1);
                self.tokens.insert(token.mint.clone(), token);
            }
        }
        self.advance();
        Ok(())
    }

    /// Promotes an already-observed candidate after an explicit future pass.
    pub fn approve(&mut self, token: DiscoveryToken) -> Result<(), ProjectionError> {
        validate_token(&token)?;
        if token.stage != DiscoveryStage::Approved {
            return Err(ProjectionError::ApproveRequiresApprovedStage);
        }

        let Some(existing) = self.tokens.get(&token.mint) else {
            return Err(ProjectionError::CandidateNotObserved { mint: token.mint });
        };
        if !same_market(existing, &token) {
            return Err(ProjectionError::ApprovalMarketMismatch { mint: token.mint });
        }
        let was_approved = existing.stage == DiscoveryStage::Approved;

        self.counters.pending = self.counters.pending.saturating_sub(1);
        self.tokens.insert(token.mint.clone(), token);
        if !was_approved {
            self.counters.approved = self.counters.approved.saturating_add(1);
        }
        self.advance();
        Ok(())
    }

    pub fn reject(
        &mut self,
        mint: &str,
        reason_code: impl Into<String>,
        seen_at_unix_ms: i64,
    ) -> Result<(), ProjectionError> {
        let reason_code = reason_code.into();
        if reason_code.trim().is_empty() {
            return Err(ProjectionError::EmptyRejectionReason);
        }

        self.counters.pending = self.counters.pending.saturating_sub(1);
        self.counters.rejected = self.counters.rejected.saturating_add(1);
        if self
            .tokens
            .remove(mint)
            .is_some_and(|token| token.stage == DiscoveryStage::Approved)
        {
            self.counters.approved = self.counters.approved.saturating_sub(1);
        }

        let summary =
            self.rejection_reasons
                .entry(reason_code.clone())
                .or_insert(RejectionSummary {
                    reason_code,
                    count: 0,
                    last_seen_unix_ms: seen_at_unix_ms,
                });
        summary.count = summary.count.saturating_add(1);
        summary.last_seen_unix_ms = summary.last_seen_unix_ms.max(seen_at_unix_ms);
        self.advance();
        Ok(())
    }

    pub fn set_mode(&mut self, mode: DiscoveryMode) {
        if self.mode != mode {
            self.mode = mode;
            self.advance();
        }
    }

    pub fn set_flow_per_minute(&mut self, flow_per_minute: Option<u64>) {
        if self.counters.flow_per_minute != flow_per_minute {
            self.counters.flow_per_minute = flow_per_minute;
            self.advance();
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> DiscoverySnapshot {
        let tokens: Vec<_> = self
            .tokens
            .values()
            .filter(|token| {
                self.mode == DiscoveryMode::ObserveAll || token.stage == DiscoveryStage::Approved
            })
            .cloned()
            .collect();

        DiscoverySnapshot {
            sequence: self.sequence,
            mode: self.mode,
            tokens_total: u64::try_from(tokens.len()).unwrap_or(u64::MAX),
            tokens_truncated: false,
            tokens,
            counters: self.counters.clone(),
            rejection_reasons: self.rejection_reasons.values().cloned().collect(),
        }
    }

    fn advance(&mut self) {
        self.sequence = self.sequence.saturating_add(1);
    }
}

fn validate_token(token: &DiscoveryToken) -> Result<(), ProjectionError> {
    validate_non_empty(&token.mint, "mint")?;
    validate_non_empty(&token.market_address, "market_address")?;
    validate_non_empty(&token.last_event_kind, "last_event_kind")?;
    validate_non_empty(&token.latest_signature, "latest_signature")?;
    validate_optional_non_empty(token.name.as_deref(), "name")?;
    validate_optional_non_empty(token.symbol.as_deref(), "symbol")?;
    validate_optional_non_empty(token.quote_mint.as_deref(), "quote_mint")?;
    validate_unsigned_decimal(
        &token.activity.base_volume_units,
        "activity.base_volume_units",
    )?;
    validate_unsigned_decimal(
        &token.activity.quote_volume_units,
        "activity.quote_volume_units",
    )?;

    if token.activity.buys.checked_add(token.activity.sells) != Some(token.activity.trades) {
        return Err(ProjectionError::InvalidTradeTotals);
    }
    if token.activity.unique_traders > token.activity.trades {
        return Err(ProjectionError::InvalidUniqueTraderTotal);
    }
    if token.first_observed_unix_ms > token.last_observed_unix_ms {
        return Err(ProjectionError::InvalidObservationTimes);
    }
    if !source_matches_venue(token.source_program, token.primary_venue) {
        return Err(ProjectionError::SourceVenueMismatch {
            source_program: token.source_program,
            venue: token.primary_venue,
        });
    }

    Ok(())
}

fn validate_non_empty(value: &str, field: &'static str) -> Result<(), ProjectionError> {
    if value.trim().is_empty() {
        Err(ProjectionError::EmptyField { field })
    } else {
        Ok(())
    }
}

fn validate_optional_non_empty(
    value: Option<&str>,
    field: &'static str,
) -> Result<(), ProjectionError> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        Err(ProjectionError::EmptyField { field })
    } else {
        Ok(())
    }
}

fn validate_unsigned_decimal(value: &str, field: &'static str) -> Result<(), ProjectionError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        Err(ProjectionError::InvalidUnsignedDecimal { field })
    } else {
        Ok(())
    }
}

const fn source_matches_venue(source_program: SourceProgram, venue: Venue) -> bool {
    matches!(
        (source_program, venue),
        (SourceProgram::Pump, Venue::PumpBondingCurve)
            | (SourceProgram::PumpSwap, Venue::PumpSwap)
            | (SourceProgram::RaydiumCpmm, Venue::RaydiumCpmm)
            | (SourceProgram::RaydiumClmm, Venue::RaydiumClmm)
            | (SourceProgram::RaydiumAmmV4, Venue::RaydiumAmmV4)
    )
}

fn same_market(existing: &DiscoveryToken, incoming: &DiscoveryToken) -> bool {
    existing.primary_venue == incoming.primary_venue
        && existing.market_address == incoming.market_address
        && existing.quote_mint == incoming.quote_mint
        && existing.source_program == incoming.source_program
}

#[cfg(test)]
mod tests {
    use soldisco_api_contracts::{
        DiscoveryActivity, DiscoveryCounters, DiscoveryMode, DiscoverySnapshot, DiscoveryStage,
        DiscoveryToken,
    };
    use soldisco_domain::{SourceProgram, Venue};

    use super::{DiscoveryProjection, ProjectionError};

    fn token(mint: &str, stage: DiscoveryStage, slot: u64) -> DiscoveryToken {
        DiscoveryToken {
            mint: mint.to_owned(),
            name: None,
            symbol: None,
            primary_venue: Venue::PumpBondingCurve,
            market_address: "curve".to_owned(),
            quote_mint: None,
            source_program: SourceProgram::Pump,
            stage,
            last_event_kind: "CREATE".to_owned(),
            observed_slot: slot,
            first_observed_unix_ms: 10,
            last_observed_unix_ms: 10,
            latest_signature: "signature".to_owned(),
            activity: DiscoveryActivity {
                trades: 0,
                buys: 0,
                sells: 0,
                unique_traders: 0,
                base_volume_units: "0".to_owned(),
                quote_volume_units: "0".to_owned(),
            },
            risk_score: None,
            opportunity_score: None,
        }
    }

    #[test]
    fn observe_all_shows_structurally_valid_unapproved_candidates() {
        let mut projection = DiscoveryProjection::default();
        projection.mark_pending();
        projection
            .observe(token("observed-mint", DiscoveryStage::Observed, 1))
            .expect("valid candidate");

        let snapshot = projection.snapshot();
        assert_eq!(snapshot.mode, DiscoveryMode::ObserveAll);
        assert_eq!(snapshot.tokens.len(), 1);
        assert_eq!(snapshot.tokens[0].stage, DiscoveryStage::Observed);
        assert_eq!(snapshot.counters.observed, 1);
        assert_eq!(snapshot.counters.approved, 0);
        assert_eq!(snapshot.counters.pending, 0);
    }

    #[test]
    fn observed_candidates_are_hidden_in_approved_only_mode() {
        let mut projection = DiscoveryProjection::default();
        projection
            .observe(token("observed-mint", DiscoveryStage::Observed, 1))
            .expect("valid candidate");
        projection.set_mode(DiscoveryMode::ApprovedOnly);

        assert!(projection.snapshot().tokens.is_empty());
    }

    #[test]
    fn approval_requires_a_previously_observed_candidate() {
        let mut projection = DiscoveryProjection::default();

        let error = projection
            .approve(token("mint", DiscoveryStage::Approved, 1))
            .expect_err("an approval cannot create a candidate");

        assert!(matches!(
            error,
            ProjectionError::CandidateNotObserved { .. }
        ));
    }

    #[test]
    fn observed_input_cannot_claim_approval() {
        let mut projection = DiscoveryProjection::default();

        let error = projection
            .observe(token("mint", DiscoveryStage::Approved, 1))
            .expect_err("observe must never imply approval");

        assert_eq!(error, ProjectionError::ObserveRequiresObservedStage);
    }

    #[test]
    fn a_new_market_requires_a_new_explicit_approval() {
        let mut projection = DiscoveryProjection::default();
        projection
            .observe(token("mint", DiscoveryStage::Observed, 1))
            .expect("candidate");
        projection
            .approve(token("mint", DiscoveryStage::Approved, 1))
            .expect("approval");

        let mut migrated = token("mint", DiscoveryStage::Observed, 2);
        migrated.primary_venue = Venue::PumpSwap;
        migrated.market_address = "pool".to_owned();
        migrated.source_program = SourceProgram::PumpSwap;
        projection
            .observe(migrated)
            .expect("new exact market observation");
        projection.set_mode(DiscoveryMode::ApprovedOnly);

        let snapshot = projection.snapshot();
        assert!(snapshot.tokens.is_empty());
        assert_eq!(snapshot.counters.approved, 0);
    }

    #[test]
    fn restore_rejects_an_inconsistent_snapshot() {
        let error = DiscoveryProjection::try_from_snapshot(DiscoverySnapshot {
            sequence: 1,
            mode: DiscoveryMode::ObserveAll,
            tokens: vec![token("mint", DiscoveryStage::Approved, 1)],
            tokens_total: 1,
            tokens_truncated: false,
            counters: DiscoveryCounters::default(),
            rejection_reasons: Vec::new(),
        })
        .expect_err("approved count must match data");

        assert_eq!(error, ProjectionError::ApprovedCounterMismatch);
    }

    #[test]
    fn rejects_malformed_activity_totals() {
        let mut malformed = token("mint", DiscoveryStage::Observed, 1);
        malformed.activity.trades = 1;

        let error = DiscoveryProjection::default()
            .observe(malformed)
            .expect_err("totals must be internally consistent");

        assert_eq!(error, ProjectionError::InvalidTradeTotals);
    }

    #[test]
    fn rejects_non_decimal_atomic_units() {
        let mut malformed = token("mint", DiscoveryStage::Observed, 1);
        malformed.activity.base_volume_units = "1.5".to_owned();

        let error = DiscoveryProjection::default()
            .observe(malformed)
            .expect_err("atomic quantities must be unsigned integers");

        assert!(matches!(
            error,
            ProjectionError::InvalidUnsignedDecimal {
                field: "activity.base_volume_units"
            }
        ));
    }
}
