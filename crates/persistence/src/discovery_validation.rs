use soldisco_api_contracts::{
    DiscoveryStage, DiscoveryToken, QualificationDecision, WindowCompleteness,
};
use soldisco_domain::{NormalizedObservation, ObservationPayload, SourceProgram, Venue};

use crate::{
    PersistenceError,
    discovery::DiscoveryProjectionMutation,
    values::{discovery_stage_name, require_non_empty},
};

pub(crate) fn ensure_discovery_work(actual: &str, work_id: i64) -> Result<(), PersistenceError> {
    if actual == "DISCOVERY" {
        Ok(())
    } else {
        Err(PersistenceError::UnexpectedWorkKind {
            work_id,
            expected: "DISCOVERY",
            actual: actual.to_owned(),
        })
    }
}

pub(crate) fn validate_token_shape(token: &DiscoveryToken) -> Result<(), PersistenceError> {
    require_non_empty(&token.mint, "token mint")?;
    require_non_empty(&token.market_address, "token market_address")?;
    require_non_empty(&token.last_event_kind, "token last_event_kind")?;
    require_non_empty(&token.latest_signature, "token latest_signature")?;
    if token
        .name
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(PersistenceError::EmptyField {
            field: "token name",
        });
    }
    if token
        .symbol
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(PersistenceError::EmptyField {
            field: "token symbol",
        });
    }
    if token
        .quote_mint
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(PersistenceError::EmptyField {
            field: "token quote_mint",
        });
    }
    validate_unsigned_decimal(
        &token.activity.base_volume_units,
        "activity.base_volume_units",
    )?;
    validate_unsigned_decimal(
        &token.activity.quote_volume_units,
        "activity.quote_volume_units",
    )?;
    if token.activity.buys.checked_add(token.activity.sells) != Some(token.activity.trades)
        || token.activity.unique_traders > token.activity.trades
        || token.first_observed_unix_ms > token.last_observed_unix_ms
        || !source_matches_venue(token.source_program, token.primary_venue)
    {
        return Err(PersistenceError::InvalidActivityTotals);
    }
    if let Some(qualification) = &token.qualification {
        for value in [
            &qualification.buy_base_volume_units,
            &qualification.sell_base_volume_units,
            &qualification.buy_quote_volume_units,
            &qualification.sell_quote_volume_units,
        ] {
            validate_unsigned_decimal(value, "qualification volume")?;
        }
        if qualification.buys.checked_add(qualification.sells) != Some(qualification.trades)
            || qualification.unique_traders > qualification.trades
            || qualification.unique_buyers > qualification.buys
            || qualification.unique_sellers > qualification.sells
            || qualification.closed_unix_ms <= qualification.opened_unix_ms
            || qualification.evaluated_unix_ms < qualification.opened_unix_ms
            || qualification
                .maximum_single_wallet_quote_share_bps
                .is_some_and(|share| share > 10_000)
        {
            return Err(PersistenceError::InvalidActivityTotals);
        }
    }
    if matches!(
        token.stage,
        DiscoveryStage::Qualified | DiscoveryStage::Approved
    ) && token.qualification.as_ref().is_none_or(|qualification| {
        qualification.decision != QualificationDecision::Pass
            || qualification.completeness != WindowCompleteness::Complete
    }) {
        return Err(PersistenceError::InvalidStoredValue {
            field: "token qualification",
            value: discovery_stage_name(token.stage).to_owned(),
        });
    }
    Ok(())
}

fn validate_unsigned_decimal(value: &str, field: &'static str) -> Result<(), PersistenceError> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        Err(PersistenceError::InvalidUnsignedDecimal { field })
    } else {
        Ok(())
    }
}

pub(crate) fn validate_token_source(
    token: &DiscoveryToken,
    observation: &NormalizedObservation,
) -> Result<(), PersistenceError> {
    let observed_at = observation
        .source_event_time_unix_ms
        .unwrap_or(observation.received_time_unix_ms);
    for (matches, field) in [
        (token.mint == observation.market.mint, "mint"),
        (
            token.primary_venue == observation.market.venue,
            "primary_venue",
        ),
        (
            token.market_address == observation.market.market_address,
            "market_address",
        ),
        (
            token.quote_mint == observation.market.quote_mint,
            "quote_mint",
        ),
        (
            token.source_program == observation.key.program,
            "source_program",
        ),
        (
            token.last_event_kind == observation.event_kind,
            "event_kind",
        ),
        (
            token.observed_slot == observation.key.coordinate.slot,
            "observed_slot",
        ),
        (
            token.latest_signature == observation.key.coordinate.signature,
            "latest_signature",
        ),
        (
            token.last_observed_unix_ms == observed_at,
            "last_observed_unix_ms",
        ),
    ] {
        if !matches {
            return Err(PersistenceError::DiscoverySourceMismatch { field });
        }
    }
    Ok(())
}

pub(crate) fn token_matches_observation_market(
    token: &DiscoveryToken,
    observation: &NormalizedObservation,
) -> bool {
    token.mint == observation.market.mint
        && token.primary_venue == observation.market.venue
        && token.market_address == observation.market.market_address
        && token.quote_mint == observation.market.quote_mint
        && token.source_program == observation.key.program
}

pub(crate) fn validate_promotion_identity(
    existing: &DiscoveryToken,
    promoted: &DiscoveryToken,
) -> Result<(), PersistenceError> {
    for (matches, field) in [
        (existing.mint == promoted.mint, "mint"),
        (
            existing.primary_venue == promoted.primary_venue,
            "primary_venue",
        ),
        (
            existing.market_address == promoted.market_address,
            "market_address",
        ),
        (existing.quote_mint == promoted.quote_mint, "quote_mint"),
        (
            existing.source_program == promoted.source_program,
            "source_program",
        ),
    ] {
        if !matches {
            return Err(PersistenceError::DiscoverySourceMismatch { field });
        }
    }
    Ok(())
}

pub(crate) fn validate_stored_stage(
    stored: &str,
    token_stage: DiscoveryStage,
) -> Result<(), PersistenceError> {
    if stored == discovery_stage_name(token_stage) {
        Ok(())
    } else {
        Err(PersistenceError::InvalidStoredValue {
            field: "discovery_tokens.stage",
            value: format!("{stored}/{}", discovery_stage_name(token_stage)),
        })
    }
}

pub(crate) fn merge_observation(
    existing: DiscoveryToken,
    mut observed: DiscoveryToken,
) -> DiscoveryToken {
    let preserve_decision = matches!(
        existing.stage,
        DiscoveryStage::Qualified | DiscoveryStage::Approved
    ) && same_market(&existing, &observed);
    observed.first_observed_unix_ms = observed
        .first_observed_unix_ms
        .min(existing.first_observed_unix_ms);
    observed.last_observed_unix_ms = observed
        .last_observed_unix_ms
        .max(existing.last_observed_unix_ms);
    observed.name = observed.name.or(existing.name);
    observed.symbol = observed.symbol.or(existing.symbol);
    if preserve_decision {
        observed.stage = existing.stage;
        observed.qualification = existing.qualification;
        observed.risk_score = observed.risk_score.or(existing.risk_score);
        observed.opportunity_score = observed.opportunity_score.or(existing.opportunity_score);
    }
    observed
}

/// A create can arrive after newer trade or migration facts during recovery.
/// It may fill missing descriptive metadata, but cannot rewind exact-market
/// identity, chain coordinates, approval state, scores, or activity.
pub(crate) fn enrich_metadata_from_stale_creation(
    existing: &mut DiscoveryToken,
    stale_candidate: &DiscoveryToken,
    observation: &NormalizedObservation,
) -> bool {
    if !matches!(observation.payload, ObservationPayload::TokenCreated { .. }) {
        return false;
    }

    let mut changed = false;
    if existing.name.is_none() && stale_candidate.name.is_some() {
        existing.name.clone_from(&stale_candidate.name);
        changed = true;
    }
    if existing.symbol.is_none() && stale_candidate.symbol.is_some() {
        existing.symbol.clone_from(&stale_candidate.symbol);
        changed = true;
    }
    changed
}

pub(crate) fn merge_approval(
    existing: DiscoveryToken,
    mut approved: DiscoveryToken,
) -> DiscoveryToken {
    if existing.observed_slot > approved.observed_slot {
        let risk_score = approved.risk_score;
        let opportunity_score = approved.opportunity_score;
        let qualification = approved.qualification;
        approved = existing;
        approved.risk_score = risk_score;
        approved.opportunity_score = opportunity_score;
        approved.qualification = qualification.or(approved.qualification);
    } else {
        approved.first_observed_unix_ms = approved
            .first_observed_unix_ms
            .min(existing.first_observed_unix_ms);
        approved.name = approved.name.or(existing.name);
        approved.symbol = approved.symbol.or(existing.symbol);
        approved.qualification = approved.qualification.or(existing.qualification);
    }
    approved.stage = DiscoveryStage::Approved;
    approved
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

pub(crate) const fn mutation_name(mutation: DiscoveryProjectionMutation) -> &'static str {
    match mutation {
        DiscoveryProjectionMutation::Inserted => "INSERTED",
        DiscoveryProjectionMutation::Updated => "UPDATED",
        DiscoveryProjectionMutation::DemotedByMarketChange => "DEMOTED_BY_MARKET_CHANGE",
        DiscoveryProjectionMutation::StaleObservationIgnored => "STALE_OBSERVATION_IGNORED",
        DiscoveryProjectionMutation::MetadataEnrichedFromStaleCreation => {
            "METADATA_ENRICHED_FROM_STALE_CREATION"
        }
        DiscoveryProjectionMutation::Promoted => "PROMOTED",
        DiscoveryProjectionMutation::AlreadyApproved => "ALREADY_APPROVED",
    }
}

#[cfg(test)]
mod tests {
    use soldisco_api_contracts::{DiscoveryActivity, DiscoveryStage, DiscoveryToken};
    use soldisco_domain::{
        ChainCoordinate, Commitment, MarketIdentity, Network, NormalizedObservation,
        ObservationKey, ObservationPayload, SourceProgram, Venue,
    };

    use super::{
        enrich_metadata_from_stale_creation, merge_observation, validate_token_shape,
        validate_token_source,
    };
    use crate::{DiscoveryObservation, PersistenceError};

    fn observation() -> NormalizedObservation {
        NormalizedObservation {
            key: ObservationKey {
                network: Network::SolanaMainnet,
                program: SourceProgram::Pump,
                coordinate: ChainCoordinate {
                    slot: 42,
                    transaction_index: Some(3),
                    signature: "signature".to_owned(),
                    instruction_index: 1,
                    event_index: 0,
                },
            },
            commitment: Commitment::Confirmed,
            schema_version: 1,
            decoder_version: "pump-v1".to_owned(),
            event_kind: "CREATE".to_owned(),
            market: MarketIdentity {
                network: Network::SolanaMainnet,
                mint: "mint".to_owned(),
                venue: Venue::PumpBondingCurve,
                market_address: "curve".to_owned(),
                quote_mint: None,
            },
            source_event_time_unix_ms: Some(100),
            received_time_unix_ms: 101,
            raw_evidence_hash: "hash".to_owned(),
            source_evidence_base64: "ZXZpZGVuY2U=".to_owned(),
            source_details: serde_json::json!({"event_type": "CREATE"}),
            payload: ObservationPayload::TokenCreated {
                name: "Token".to_owned(),
                symbol: "TOK".to_owned(),
                uri: "https://example.invalid/token.json".to_owned(),
                creator: "creator".to_owned(),
                user: "user".to_owned(),
            },
        }
    }

    fn token(stage: DiscoveryStage) -> DiscoveryToken {
        DiscoveryToken {
            mint: "mint".to_owned(),
            name: Some("Token".to_owned()),
            symbol: Some("TOK".to_owned()),
            primary_venue: Venue::PumpBondingCurve,
            market_address: "curve".to_owned(),
            quote_mint: None,
            source_program: SourceProgram::Pump,
            stage,
            last_event_kind: "CREATE".to_owned(),
            observed_slot: 42,
            first_observed_unix_ms: 100,
            last_observed_unix_ms: 100,
            latest_signature: "signature".to_owned(),
            activity: DiscoveryActivity {
                trades: 0,
                buys: 0,
                sells: 0,
                unique_traders: 0,
                base_volume_units: "0".to_owned(),
                quote_volume_units: "0".to_owned(),
            },
            qualification: None,
            risk_score: None,
            opportunity_score: None,
        }
    }

    #[test]
    fn observed_candidate_is_bound_to_durable_chain_coordinates() {
        validate_token_source(&token(DiscoveryStage::Observed), &observation())
            .expect("matching coordinates");

        let mut mismatched = token(DiscoveryStage::Observed);
        mismatched.latest_signature = "other".to_owned();
        assert!(matches!(
            validate_token_source(&mismatched, &observation()),
            Err(PersistenceError::DiscoverySourceMismatch {
                field: "latest_signature"
            })
        ));
    }

    #[test]
    fn malformed_totals_do_not_enter_the_projection() {
        let mut malformed = token(DiscoveryStage::Observed);
        malformed.activity.trades = 1;

        assert!(matches!(
            validate_token_shape(&malformed),
            Err(PersistenceError::InvalidActivityTotals)
        ));
    }

    #[test]
    fn later_observation_does_not_demote_an_approved_token() {
        let mut approved = token(DiscoveryStage::Approved);
        approved.risk_score = soldisco_domain::Score::new(5).ok();
        let merged = merge_observation(approved, token(DiscoveryStage::Observed));

        assert_eq!(merged.stage, DiscoveryStage::Approved);
        assert_eq!(
            merged.risk_score.map(soldisco_domain::Score::value),
            Some(5)
        );
    }

    #[test]
    fn observation_on_a_different_market_does_not_inherit_approval() {
        let mut approved = token(DiscoveryStage::Approved);
        approved.risk_score = soldisco_domain::Score::new(5).ok();
        let mut migrated = token(DiscoveryStage::Observed);
        migrated.primary_venue = Venue::PumpSwap;
        migrated.market_address = "pool".to_owned();
        migrated.source_program = SourceProgram::PumpSwap;

        let merged = merge_observation(approved, migrated);

        assert_eq!(merged.stage, DiscoveryStage::Observed);
        assert_eq!(merged.risk_score, None);
    }

    #[test]
    fn stale_create_only_enriches_missing_metadata() {
        let mut retained = token(DiscoveryStage::Observed);
        retained.name = None;
        retained.symbol = None;
        retained.primary_venue = Venue::PumpSwap;
        retained.market_address = "newer-pool".to_owned();
        retained.quote_mint = Some("wrapped-sol".to_owned());
        retained.source_program = SourceProgram::PumpSwap;
        retained.observed_slot = 100;
        retained.latest_signature = "newer-signature".to_owned();
        retained.activity.trades = 1;
        retained.activity.buys = 1;
        retained.activity.base_volume_units = "25".to_owned();
        retained.activity.quote_volume_units = "50".to_owned();

        let stale = token(DiscoveryStage::Observed);
        assert!(enrich_metadata_from_stale_creation(
            &mut retained,
            &stale,
            &observation(),
        ));
        assert_eq!(retained.name.as_deref(), Some("Token"));
        assert_eq!(retained.symbol.as_deref(), Some("TOK"));
        assert_eq!(retained.primary_venue, Venue::PumpSwap);
        assert_eq!(retained.market_address, "newer-pool");
        assert_eq!(retained.observed_slot, 100);
        assert_eq!(retained.latest_signature, "newer-signature");
        assert_eq!(retained.activity.trades, 1);
        assert_eq!(retained.activity.base_volume_units, "25");
    }

    #[test]
    fn observation_type_requires_explicit_validation_provenance() {
        let candidate = DiscoveryObservation {
            token: token(DiscoveryStage::Observed),
            validation_source: "PUMP_STRUCTURAL_DECODER".to_owned(),
            validation_version: "1".to_owned(),
        };

        assert_eq!(candidate.token.stage, DiscoveryStage::Observed);
        assert!(!candidate.validation_source.is_empty());
    }
}
