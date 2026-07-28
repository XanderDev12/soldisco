//! PostgreSQL ownership boundary for the Rust backend.

use std::time::Duration;

use serde_json::Value;
use soldisco_domain::{Commitment, Network, NormalizedObservation, SourceProgram, Venue};
use sqlx::{PgPool, postgres::PgPoolOptions};
use thiserror::Error;

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("observation serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("observation key network does not match market network")]
    NetworkMismatch,
    #[error("{field} cannot be represented by PostgreSQL BIGINT")]
    ValueOutOfRange { field: &'static str },
}

impl Database {
    pub async fn connect(
        database_url: &str,
        max_connections: u32,
    ) -> Result<Self, PersistenceError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(10))
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), PersistenceError> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn health(&self) -> Result<(), PersistenceError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn stream_requested_running(&self) -> Result<bool, PersistenceError> {
        let requested = sqlx::query_scalar::<_, bool>(
            "SELECT requested_running FROM stream_control WHERE singleton = TRUE",
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(requested)
    }

    pub async fn set_stream_requested_running(
        &self,
        requested_running: bool,
    ) -> Result<(), PersistenceError> {
        sqlx::query(
            "UPDATE stream_control \
             SET requested_running = $1, updated_at = NOW() \
             WHERE singleton = TRUE",
        )
        .bind(requested_running)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Inserts a normalized chain fact exactly once.
    ///
    /// Returns `true` for a new row and `false` when the durable chain identity
    /// was already present. Finality and correction handling will be added with
    /// the live recovery implementation.
    pub async fn insert_observation(
        &self,
        observation: &NormalizedObservation,
    ) -> Result<bool, PersistenceError> {
        if observation.key.network != observation.market.network {
            return Err(PersistenceError::NetworkMismatch);
        }

        let slot = to_i64(observation.key.coordinate.slot, "slot")?;
        let normalized: Value = serde_json::to_value(observation)?;
        let network = network_name(observation.key.network);
        let source_program = source_program_name(observation.key.program);
        let commitment = commitment_name(observation.commitment);
        let venue = venue_name(observation.market.venue);

        let mut transaction = self.pool.begin().await?;
        let inserted_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO chain_observations (\
                network, source_program, slot, signature, instruction_index, \
                event_index, commitment, schema_version, decoder_version, \
                event_kind, mint, venue, market_address, quote_mint, \
                source_event_time_unix_ms, received_time_unix_ms, \
                raw_evidence_hash, normalized_observation\
             ) VALUES (\
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, \
                $14, $15, $16, $17, $18\
             ) ON CONFLICT (\
                network, source_program, signature, instruction_index, event_index\
             ) DO NOTHING \
             RETURNING id",
        )
        .bind(network)
        .bind(source_program)
        .bind(slot)
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
        }

        transaction.commit().await?;
        Ok(inserted_id.is_some())
    }
}

fn to_i64(value: u64, field: &'static str) -> Result<i64, PersistenceError> {
    i64::try_from(value).map_err(|_| PersistenceError::ValueOutOfRange { field })
}

const fn network_name(network: Network) -> &'static str {
    match network {
        Network::SolanaMainnet => "SOLANA_MAINNET",
        Network::SolanaDevnet => "SOLANA_DEVNET",
    }
}

const fn source_program_name(program: SourceProgram) -> &'static str {
    match program {
        SourceProgram::Pump => "PUMP",
        SourceProgram::PumpSwap => "PUMP_SWAP",
        SourceProgram::RaydiumCpmm => "RAYDIUM_CPMM",
        SourceProgram::RaydiumClmm => "RAYDIUM_CLMM",
        SourceProgram::RaydiumAmmV4 => "RAYDIUM_AMM_V4",
    }
}

const fn commitment_name(commitment: Commitment) -> &'static str {
    match commitment {
        Commitment::Processed => "PROCESSED",
        Commitment::Confirmed => "CONFIRMED",
        Commitment::Finalized => "FINALIZED",
    }
}

const fn venue_name(venue: Venue) -> &'static str {
    match venue {
        Venue::PumpBondingCurve => "PUMP_BONDING_CURVE",
        Venue::PumpSwap => "PUMP_SWAP",
        Venue::RaydiumCpmm => "RAYDIUM_CPMM",
        Venue::RaydiumClmm => "RAYDIUM_CLMM",
        Venue::RaydiumAmmV4 => "RAYDIUM_AMM_V4",
    }
}

#[cfg(test)]
mod tests {
    use super::{PersistenceError, to_i64};

    #[test]
    fn rejects_unsigned_values_postgres_cannot_represent() {
        let error = to_i64(u64::MAX, "slot").expect_err("u64::MAX must not fit");

        assert!(matches!(
            error,
            PersistenceError::ValueOutOfRange { field: "slot" }
        ));
    }

    #[test]
    fn migration_contains_durable_chain_identity() {
        let migration = include_str!("../migrations/0001_foundation.sql");

        assert!(migration.contains("signature"));
        assert!(migration.contains("instruction_index"));
        assert!(migration.contains("event_index"));
        assert!(migration.contains("observation_work"));
        assert!(migration.contains("recovery_checkpoints"));
    }
}
