use std::{sync::Arc, time::Duration};

use soldisco_api_contracts::{DiscoveryActivity, DiscoveryStage, DiscoveryToken};
use soldisco_domain::{NormalizedObservation, ObservationPayload};
use soldisco_persistence::{
    Database, DiscoveryObservation, PersistenceError, WorkFailureDisposition,
};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::state::LiveEventBus;

const WORK_KIND: &str = "DISCOVERY";
const VALIDATION_SOURCE: &str = "PUMP_STRUCTURAL_DECODER";

#[derive(Clone, Debug)]
pub struct DiscoveryWorkerConfig {
    pub worker_id: String,
    pub claim_batch: u32,
    pub lease_duration: Duration,
    pub retry_after: Duration,
    pub maximum_attempts: u32,
    pub idle_poll_interval: Duration,
}

impl DiscoveryWorkerConfig {
    #[must_use]
    pub fn for_process() -> Self {
        Self {
            worker_id: format!(
                "soldisco-discovery-{}-{}",
                std::process::id(),
                unix_time_millis()
            ),
            claim_batch: 64,
            lease_duration: Duration::from_secs(30),
            retry_after: Duration::from_millis(500),
            maximum_attempts: 5,
            idle_poll_interval: Duration::from_millis(250),
        }
    }
}

pub async fn run_discovery_worker(
    database: Database,
    events: LiveEventBus,
    wake: Arc<Notify>,
    cancellation: CancellationToken,
    config: DiscoveryWorkerConfig,
) -> Result<(), PersistenceError> {
    loop {
        if cancellation.is_cancelled() {
            return Ok(());
        }

        let claimed = database
            .claim_observation_work(
                WORK_KIND,
                &config.worker_id,
                config.claim_batch,
                config.lease_duration,
            )
            .await?;

        if claimed.is_empty() {
            tokio::select! {
                () = cancellation.cancelled() => return Ok(()),
                () = wake.notified() => {}
                () = tokio::time::sleep(config.idle_poll_interval) => {}
            }
            continue;
        }

        for work in claimed {
            if cancellation.is_cancelled() {
                return Ok(());
            }

            if let Err(error) = database
                .renew_observation_work_lease(
                    work.work_id,
                    &config.worker_id,
                    config.lease_duration,
                )
                .await
            {
                if matches!(error, PersistenceError::WorkLeaseLost { .. }) {
                    tracing::debug!(
                        work_id = work.work_id,
                        "discovery work lease expired before processing and will be reclaimed"
                    );
                    continue;
                }
                return Err(error);
            }

            let token = discovery_token(&work.observation);
            let outcome = database
                .commit_discovery_observation(
                    work.work_id,
                    &config.worker_id,
                    &DiscoveryObservation {
                        token,
                        validation_source: VALIDATION_SOURCE.to_owned(),
                        validation_version: work.observation.decoder_version.clone(),
                    },
                )
                .await;

            match outcome {
                Ok(_) => events.publish_discovery_projection_changed(),
                Err(PersistenceError::WorkLeaseLost { .. }) => {
                    tracing::debug!(
                        work_id = work.work_id,
                        "discovery work lease was reclaimed while processing"
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        work_id = work.work_id,
                        attempts = work.attempts,
                        %error,
                        "discovery observation could not be committed"
                    );
                    let disposition = match database
                        .retry_or_fail_observation_work(
                            work.work_id,
                            &config.worker_id,
                            persistence_error_code(&error),
                            config.retry_after,
                            config.maximum_attempts,
                        )
                        .await
                    {
                        Ok(disposition) => disposition,
                        Err(PersistenceError::WorkLeaseLost { .. }) => {
                            tracing::debug!(
                                work_id = work.work_id,
                                "failed discovery work lease was already reclaimed"
                            );
                            continue;
                        }
                        Err(retry_error) => return Err(retry_error),
                    };
                    if disposition == WorkFailureDisposition::PermanentlyFailed {
                        events.publish_discovery_projection_changed();
                    }
                }
            }
        }
    }
}

fn discovery_token(observation: &NormalizedObservation) -> DiscoveryToken {
    let (name, symbol) = match &observation.payload {
        ObservationPayload::TokenCreated { name, symbol, .. } => {
            (non_empty(name), non_empty(symbol))
        }
        _ => (None, None),
    };
    let observed_at = observation
        .source_event_time_unix_ms
        .unwrap_or(observation.received_time_unix_ms);

    DiscoveryToken {
        mint: observation.market.mint.clone(),
        name,
        symbol,
        primary_venue: observation.market.venue,
        market_address: observation.market.market_address.clone(),
        quote_mint: observation.market.quote_mint.clone(),
        source_program: observation.key.program,
        stage: DiscoveryStage::Observed,
        last_event_kind: observation.event_kind.clone(),
        observed_slot: observation.key.coordinate.slot,
        first_observed_unix_ms: observed_at,
        last_observed_unix_ms: observed_at,
        latest_signature: observation.key.coordinate.signature.clone(),
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

fn non_empty(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn persistence_error_code(error: &PersistenceError) -> &'static str {
    match error {
        PersistenceError::Database(_) => "DISCOVERY_DATABASE_ERROR",
        PersistenceError::Serialization(_) => "DISCOVERY_SERIALIZATION_ERROR",
        PersistenceError::WorkLeaseLost { .. } => "DISCOVERY_WORK_LEASE_LOST",
        PersistenceError::DiscoverySourceMismatch { .. } => "DISCOVERY_SOURCE_MISMATCH",
        PersistenceError::InvalidActivityTotals
        | PersistenceError::InvalidUnsignedDecimal { .. }
        | PersistenceError::EmptyField { .. } => "DISCOVERY_INVALID_CANDIDATE",
        _ => "DISCOVERY_PROCESSING_ERROR",
    }
}

fn unix_time_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use soldisco_domain::{
        ChainCoordinate, Commitment, MarketIdentity, Network, NormalizedObservation,
        ObservationKey, ObservationPayload, SourceProgram, Venue,
    };

    use super::discovery_token;

    #[test]
    fn structural_discovery_never_invents_scores_or_approval() {
        let token = discovery_token(&NormalizedObservation {
            key: ObservationKey {
                network: Network::SolanaMainnet,
                program: SourceProgram::Pump,
                coordinate: ChainCoordinate {
                    slot: 1,
                    transaction_index: None,
                    signature: "signature".to_owned(),
                    instruction_index: 0,
                    event_index: 0,
                },
            },
            commitment: Commitment::Confirmed,
            schema_version: 1,
            decoder_version: "decoder".to_owned(),
            event_kind: "CREATE".to_owned(),
            market: MarketIdentity {
                network: Network::SolanaMainnet,
                mint: "mint".to_owned(),
                venue: Venue::PumpBondingCurve,
                market_address: "curve".to_owned(),
                quote_mint: Some("quote".to_owned()),
            },
            source_event_time_unix_ms: Some(1_000),
            received_time_unix_ms: 2_000,
            raw_evidence_hash: "hash".to_owned(),
            source_evidence_base64: "ZXZpZGVuY2U=".to_owned(),
            payload: ObservationPayload::TokenCreated {
                name: " Token ".to_owned(),
                symbol: " TOK ".to_owned(),
                uri: "uri".to_owned(),
                creator: "creator".to_owned(),
                user: "user".to_owned(),
            },
        });

        assert_eq!(
            token.stage,
            soldisco_api_contracts::DiscoveryStage::Observed
        );
        assert_eq!(token.name.as_deref(), Some("Token"));
        assert_eq!(token.symbol.as_deref(), Some("TOK"));
        assert_eq!(token.risk_score, None);
        assert_eq!(token.opportunity_score, None);
    }
}
