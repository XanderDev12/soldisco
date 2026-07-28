use serde_json::{Value, json};
use soldisco_domain::NormalizedObservation;

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
    /// identity returns `false` without creating work or changing counters.
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
        .bind(network)
        .bind(source_program)
        .bind(slot)
        .bind(transaction_index)
        .bind(&observation.key.coordinate.signature)
        .bind(i32::from(observation.key.coordinate.instruction_index))
        .bind(i32::from(observation.key.coordinate.event_index))
        .bind(commitment)
        .bind(i32::from(observation.schema_version))
        .bind(&observation.decoder_version)
        .bind(&observation.event_kind)
        .bind(&observation.market.mint)
        .bind(venue)
        .bind(&observation.market.market_address)
        .bind(&observation.market.quote_mint)
        .bind(observation.source_event_time_unix_ms)
        .bind(observation.received_time_unix_ms)
        .bind(&observation.raw_evidence_hash)
        .bind(normalized)
        .fetch_optional(&mut *transaction)
        .await?;

        if let Some(observation_id) = inserted_id {
            sqlx::query(
                "INSERT INTO observation_work (observation_id, work_kind) \
                 VALUES ($1, 'DISCOVERY')",
            )
            .bind(observation_id)
            .execute(&mut *transaction)
            .await?;

            let sequence = sqlx::query_scalar::<_, i64>(
                "UPDATE discovery_projection_state \
                 SET pending = pending + 1, sequence = sequence + 1, \
                     updated_at = NOW() \
                 WHERE singleton = TRUE \
                 RETURNING sequence",
            )
            .fetch_one(&mut *transaction)
            .await?;

            sqlx::query(
                "INSERT INTO projection_events (projection_name, event_kind, payload) \
                 VALUES ('DISCOVERY', 'DISCOVERY_PENDING_CHANGED', $1)",
            )
            .bind(json!({
                "projection_sequence": sequence,
                "mint": observation.market.mint,
                "observation_id": observation_id,
            }))
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(inserted_id.is_some())
    }
}

#[cfg(test)]
mod tests {
    use crate::{PersistenceError, values::to_i64};

    #[test]
    fn rejects_unsigned_values_postgres_cannot_represent() {
        let error = to_i64(u64::MAX, "slot").expect_err("u64::MAX must not fit");

        assert!(matches!(
            error,
            PersistenceError::ValueOutOfRange { field: "slot" }
        ));
    }
}
