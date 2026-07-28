//! PostgreSQL ownership boundary for the Rust backend.
//!
//! SQL is intentionally confined to this crate. The public APIs expose
//! durable observation ingestion, leased work, recovery checkpoints,
//! screening audit records, and the browser-facing discovery projection.

mod checkpoints;
mod collection_gaps;
mod discovery;
mod discovery_activity;
mod discovery_storage;
mod discovery_validation;
mod error;
mod markets;
mod network_binding;
mod observations;
mod prefilter_defaults;
mod pump_swap;
mod quarantine;
mod retention;
mod screening;
mod values;
mod work;

use std::time::Duration;

use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};

pub use checkpoints::RecoveryCheckpoint;
pub use collection_gaps::{CollectionGap, CollectionPosition, NewCollectionGap};
pub use discovery::{DiscoveryObservation, DiscoveryProjectionMutation, DiscoveryRejection};
pub use error::PersistenceError;
pub use prefilter_defaults::StoredPrefilterDefaults;
pub use pump_swap::PumpSwapPool;
pub use quarantine::{
    IntakeQuarantineRecord, MAX_QUARANTINE_EVIDENCE_BASE64_BYTES, StoredIntakeQuarantine,
};
pub use retention::{MAX_RETENTION_BATCH_SIZE, RetentionPolicy, RetentionPruneResult};
pub use screening::ScreeningRun;
pub use work::{ClaimedObservationWork, WorkFailureDisposition};

#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(
        database_url: &str,
        max_connections: u32,
    ) -> Result<Self, PersistenceError> {
        let options = database_url.parse::<PgConnectOptions>()?;
        Self::connect_with_options(options, max_connections).await
    }

    pub async fn connect_with_options(
        options: PgConnectOptions,
        max_connections: u32,
    ) -> Result<Self, PersistenceError> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .acquire_timeout(Duration::from_secs(10))
            .connect_with(options)
            .await?;

        Ok(Self { pool })
    }

    pub async fn close(self) {
        self.pool.close().await;
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
}

#[cfg(test)]
mod tests {
    #[test]
    fn migrations_preserve_identity_and_add_leased_projection_work() {
        let foundation = include_str!("../migrations/0001_foundation.sql");
        let pipeline = include_str!("../migrations/0002_durable_discovery_pipeline.sql");
        let hardening = include_str!("../migrations/0003_persistence_hardening.sql");
        let prefilter_defaults = include_str!("../migrations/0004_prefilter_defaults.sql");

        assert!(foundation.contains("signature"));
        assert!(foundation.contains("instruction_index"));
        assert!(foundation.contains("event_index"));
        assert!(foundation.contains("observation_work"));
        assert!(foundation.contains("recovery_checkpoints"));
        assert!(pipeline.contains("lease_owner"));
        assert!(pipeline.contains("lease_expires_at"));
        assert!(pipeline.contains("discovery_projection_state"));
        assert!(pipeline.contains("discovery_tokens"));
        assert!(pipeline.contains("OBSERVE_ALL"));
        assert!(hardening.contains("database_network_binding"));
        assert!(hardening.contains("intake_quarantine"));
        assert!(hardening.contains("collection_gaps"));
        assert!(hardening.contains("transaction_index"));
        assert!(hardening.contains("observation_work_terminal_retention_idx"));
        assert!(hardening.contains("soldisco_enforce_bound_network"));
        assert!(prefilter_defaults.contains("CREATE TABLE prefilter_defaults"));
        assert!(prefilter_defaults.contains("revision BIGINT"));
        assert!(prefilter_defaults.contains("rpc_request_timeout_ms <= max_event_age_ms"));
        assert!(
            !prefilter_defaults.contains("INSERT INTO prefilter_defaults"),
            "validated environment values seed this table exactly once at runtime"
        );
    }
}
