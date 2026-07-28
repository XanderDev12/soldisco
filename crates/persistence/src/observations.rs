use serde_json::{Value, json};
use soldisco_domain::NormalizedObservation;
use sqlx::{Postgres, Transaction};

use crate::{
    Database, PersistenceError,
    quarantine::validate_evidence_base64,
    values::{commitment_name, network_name, source_program_name, to_i64, venue_name},
};

impl Database {
    /// Inserts a normalized chain fact and its discovery work exactly once.
    ///
    /// The observation, work item, pending counter, and projection invalidation
    /// are committed in one transaction. A replay of the same durable chain
    /// identity and exact immutable evidence returns `false` without creating
    /// work or changing counters. Reusing an identity with divergent evidence
    /// fails closed.
    pub async fn insert_observation(
        &self,
        observation: &NormalizedObservation,
    ) -> Result<bool, PersistenceError> {
        if observation.key.network != observation.market.network {
            return Err(PersistenceError::NetworkMismatch);
        }
        validate_evidence_base64(
            &observation.source_evidence_base64,
            "source_evidence_base64",
        )?;

        let slot = to_i64(observation.key.coordinate.slot, "slot")?;
        let transaction_index = observation
            .key
            .coordinate
            .transaction_index
            .map(|value| to_i64(value, "transaction_index"))
            .transpose()?;
        let normalized: Value = serde_json::to_value(observation)?;
        let network = network_name(observation.key.network);
        let source_program = source_program_name(observation.key.program);
        let commitment = commitment_name(observation.commitment);
        let venue = venue_name(observation.market.venue);

        let mut transaction = self.pool.begin().await?;
        let inserted = insert_chain_observation(
            &mut transaction,
            observation,
            PreparedObservation {
                slot,
                transaction_index,
                normalized,
                network,
                source_program,
                commitment,
                venue,
            },
        )
        .await?;

        if inserted.is_new {
            enqueue_discovery_work(&mut transaction, observation, inserted.id).await?;
        }

        transaction.commit().await?;
        Ok(inserted.is_new)
    }
}

pub(crate) struct ObservationInsert {
    pub id: i64,
    pub is_new: bool,
}

struct PreparedObservation<'a> {
    slot: i64,
    transaction_index: Option<i64>,
    normalized: Value,
    network: &'a str,
    source_program: &'a str,
    commitment: &'a str,
    venue: &'a str,
}

pub(crate) async fn insert_chain_observation_unprepared(
    transaction: &mut Transaction<'_, Postgres>,
    observation: &NormalizedObservation,
) -> Result<ObservationInsert, PersistenceError> {
    if observation.key.network != observation.market.network {
        return Err(PersistenceError::NetworkMismatch);
    }
    validate_evidence_base64(
        &observation.source_evidence_base64,
        "source_evidence_base64",
    )?;
    let prepared = PreparedObservation {
        slot: to_i64(observation.key.coordinate.slot, "slot")?,
        transaction_index: observation
            .key
            .coordinate
            .transaction_index
            .map(|value| to_i64(value, "transaction_index"))
            .transpose()?,
        normalized: serde_json::to_value(observation)?,
        network: network_name(observation.key.network),
        source_program: source_program_name(observation.key.program),
        commitment: commitment_name(observation.commitment),
        venue: venue_name(observation.market.venue),
    };
    insert_chain_observation(transaction, observation, prepared).await
}

async fn insert_chain_observation(
    transaction: &mut Transaction<'_, Postgres>,
    observation: &NormalizedObservation,
    prepared: PreparedObservation<'_>,
) -> Result<ObservationInsert, PersistenceError> {
    let inserted_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO chain_observations (\
            network, source_program, slot, transaction_index, signature, \
            instruction_index, event_index, commitment, schema_version, \
            decoder_version, event_kind, mint, venue, market_address, quote_mint, \
            source_event_time_unix_ms, received_time_unix_ms, \
            raw_evidence_hash, normalized_observation\
         ) VALUES (\
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, \
            $14, $15, $16, $17, $18, $19\
         ) ON CONFLICT (\
            network, source_program, signature, instruction_index, event_index\
         ) DO NOTHING \
         RETURNING id",
    )
    .bind(prepared.network)
    .bind(prepared.source_program)
    .bind(prepared.slot)
    .bind(prepared.transaction_index)
    .bind(&observation.key.coordinate.signature)
    .bind(i32::from(observation.key.coordinate.instruction_index))
    .bind(i32::from(observation.key.coordinate.event_index))
    .bind(prepared.commitment)
    .bind(i32::from(observation.schema_version))
    .bind(&observation.decoder_version)
    .bind(&observation.event_kind)
    .bind(&observation.market.mint)
    .bind(prepared.venue)
    .bind(&observation.market.market_address)
    .bind(&observation.market.quote_mint)
    .bind(observation.source_event_time_unix_ms)
    .bind(observation.received_time_unix_ms)
    .bind(&observation.raw_evidence_hash)
    .bind(&prepared.normalized)
    .fetch_optional(&mut **transaction)
    .await?;

    match inserted_id {
        Some(id) => Ok(ObservationInsert { id, is_new: true }),
        None => {
            let (id, stored_normalized) = sqlx::query_as::<_, (i64, Value)>(
                "SELECT id, normalized_observation \
                 FROM chain_observations \
                 WHERE network = $1 \
                   AND source_program = $2 \
                   AND signature = $3 \
                   AND instruction_index = $4 \
                   AND event_index = $5 \
                 FOR UPDATE",
            )
            .bind(prepared.network)
            .bind(prepared.source_program)
            .bind(&observation.key.coordinate.signature)
            .bind(i32::from(observation.key.coordinate.instruction_index))
            .bind(i32::from(observation.key.coordinate.event_index))
            .fetch_one(&mut **transaction)
            .await?;
            let mut stored: NormalizedObservation = serde_json::from_value(stored_normalized)?;
            if !compatible_replay(&stored, observation) {
                return Err(PersistenceError::ObservationEvidenceConflict {
                    network: prepared.network.to_owned(),
                    source_program: prepared.source_program.to_owned(),
                    signature: observation.key.coordinate.signature.clone(),
                    instruction_index: observation.key.coordinate.instruction_index,
                    event_index: observation.key.coordinate.event_index,
                });
            }
            if observation.received_time_unix_ms < stored.received_time_unix_ms {
                let opens_window = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS (\
                        SELECT 1 \
                        FROM discovery_windows \
                        WHERE opening_observation_id = $1\
                     )",
                )
                .bind(id)
                .fetch_one(&mut **transaction)
                .await?;
                if !opens_window {
                    stored.received_time_unix_ms = observation.received_time_unix_ms;
                    sqlx::query(
                        "UPDATE chain_observations \
                         SET received_time_unix_ms = $2, normalized_observation = $3 \
                         WHERE id = $1",
                    )
                    .bind(id)
                    .bind(stored.received_time_unix_ms)
                    .bind(serde_json::to_value(&stored)?)
                    .execute(&mut **transaction)
                    .await?;
                    sqlx::query(
                        "UPDATE discovery_window_observations \
                         SET admitted_at_unix_ms = LEAST(admitted_at_unix_ms, $2) \
                         WHERE observation_id = $1",
                    )
                    .bind(id)
                    .bind(stored.received_time_unix_ms)
                    .execute(&mut **transaction)
                    .await?;
                }
            }
            Ok(ObservationInsert { id, is_new: false })
        }
    }
}

/// Defines which delivery metadata may vary when an immutable chain event is
/// replayed under the same durable identity.
///
/// The replay transport policy is:
/// - receipt order may differ; persistence keeps the earliest receipt
///   atomically, except that an existing window's opening observation is
///   frozen because changing it would invalidate the window's fixed bounds;
/// - commitment may arrive in either order and is accepted without upgrading
///   or downgrading the stored fact;
/// - a provider may add or omit `transaction_index`, but two different known
///   indexes conflict.
///
/// Every decoded payload, evidence, source-detail and provenance field remains
/// exact. Comparing these fields explicitly avoids treating the receipt-time
/// and commitment members inside `normalized_observation` as immutable.
fn compatible_replay(stored: &NormalizedObservation, incoming: &NormalizedObservation) -> bool {
    let transaction_indexes_compatible = match (
        stored.key.coordinate.transaction_index,
        incoming.key.coordinate.transaction_index,
    ) {
        (Some(stored), Some(incoming)) => stored == incoming,
        (None, _) | (_, None) => true,
    };
    if !transaction_indexes_compatible {
        return false;
    }

    let mut canonical_incoming = incoming.clone();
    canonical_incoming.received_time_unix_ms = stored.received_time_unix_ms;
    canonical_incoming.commitment = stored.commitment;
    canonical_incoming.key.coordinate.transaction_index = stored.key.coordinate.transaction_index;
    canonical_incoming == *stored
}

pub(crate) async fn enqueue_discovery_work(
    transaction: &mut Transaction<'_, Postgres>,
    observation: &NormalizedObservation,
    observation_id: i64,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO observation_work (observation_id, work_kind) \
         VALUES ($1, 'DISCOVERY')",
    )
    .bind(observation_id)
    .execute(&mut **transaction)
    .await?;

    let sequence = sqlx::query_scalar::<_, i64>(
        "UPDATE discovery_projection_state \
         SET queued_facts = queued_facts + 1, sequence = sequence + 1, \
             updated_at = NOW() \
         WHERE singleton = TRUE \
         RETURNING sequence",
    )
    .fetch_one(&mut **transaction)
    .await?;

    sqlx::query(
        "INSERT INTO projection_events (projection_name, event_kind, payload) \
         VALUES ('DISCOVERY', 'DISCOVERY_QUEUED_FACTS_CHANGED', $1)",
    )
    .bind(json!({
        "projection_sequence": sequence,
        "mint": observation.market.mint,
        "observation_id": observation_id,
    }))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use soldisco_domain::{
        ChainCoordinate, Commitment, MarketIdentity, Network, NormalizedObservation,
        ObservationKey, ObservationPayload, SourceProgram, Venue,
    };

    use super::compatible_replay;
    use crate::{PersistenceError, values::to_i64};

    fn observation() -> NormalizedObservation {
        NormalizedObservation {
            key: ObservationKey {
                network: Network::SolanaMainnet,
                program: SourceProgram::Pump,
                coordinate: ChainCoordinate {
                    slot: 42,
                    transaction_index: None,
                    signature: "signature".to_owned(),
                    instruction_index: 1,
                    event_index: 2,
                },
            },
            commitment: Commitment::Processed,
            schema_version: 1,
            decoder_version: "decoder-v1".to_owned(),
            event_kind: "CREATE".to_owned(),
            market: MarketIdentity {
                network: Network::SolanaMainnet,
                mint: "mint".to_owned(),
                venue: Venue::PumpBondingCurve,
                market_address: "market".to_owned(),
                quote_mint: Some("quote".to_owned()),
            },
            source_event_time_unix_ms: Some(40),
            received_time_unix_ms: 100,
            raw_evidence_hash: "hash".to_owned(),
            source_evidence_base64: "ZXZpZGVuY2U=".to_owned(),
            source_details: serde_json::json!({"source": "pump"}),
            payload: ObservationPayload::TokenCreated {
                name: "Token".to_owned(),
                symbol: "TOK".to_owned(),
                uri: "https://example.invalid/token.json".to_owned(),
                creator: "creator".to_owned(),
                user: "user".to_owned(),
            },
        }
    }

    #[test]
    fn rejects_unsigned_values_postgres_cannot_represent() {
        let error = to_i64(u64::MAX, "slot").expect_err("u64::MAX must not fit");

        assert!(matches!(
            error,
            PersistenceError::ValueOutOfRange { field: "slot" }
        ));
    }

    #[test]
    fn replay_transport_metadata_can_arrive_later_without_changing_evidence() {
        let stored = observation();
        let mut replay = stored.clone();
        replay.received_time_unix_ms = 150;
        replay.commitment = Commitment::Finalized;
        replay.key.coordinate.transaction_index = Some(7);

        assert!(compatible_replay(&stored, &replay));
    }

    #[test]
    fn compatible_replay_can_report_an_earlier_transport_receipt() {
        let stored = observation();
        let mut replay = stored.clone();
        replay.received_time_unix_ms = 99;

        assert!(compatible_replay(&stored, &replay));
    }

    #[test]
    fn known_transaction_indexes_must_not_diverge() {
        let mut stored = observation();
        stored.key.coordinate.transaction_index = Some(7);
        let mut replay = stored.clone();
        replay.key.coordinate.transaction_index = Some(8);

        assert!(!compatible_replay(&stored, &replay));
    }

    #[test]
    fn normalized_payload_and_source_provenance_remain_immutable() {
        let stored = observation();
        let mut payload_replay = stored.clone();
        payload_replay.payload = ObservationPayload::TokenCreated {
            name: "Different".to_owned(),
            symbol: "TOK".to_owned(),
            uri: "https://example.invalid/token.json".to_owned(),
            creator: "creator".to_owned(),
            user: "user".to_owned(),
        };
        let mut provenance_replay = stored.clone();
        provenance_replay.source_details = serde_json::json!({"source": "other"});

        assert!(!compatible_replay(&stored, &payload_replay));
        assert!(!compatible_replay(&stored, &provenance_replay));
    }
}
