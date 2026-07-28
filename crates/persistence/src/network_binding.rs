use soldisco_domain::Network;

use crate::{
    Database, PersistenceError,
    values::{network_name, parse_network},
};

impl Database {
    /// Atomically binds an otherwise empty database to exactly one Solana
    /// network. Rebinding to the same network is idempotent; a different
    /// network fails before collectors can ingest anything.
    ///
    /// Returns `true` only when this call established the binding.
    pub async fn bind_network(&self, network: Network) -> Result<bool, PersistenceError> {
        let requested = network_name(network);
        let mut transaction = self.pool.begin().await?;
        let inserted = sqlx::query_scalar::<_, String>(
            "INSERT INTO database_network_binding (singleton, network) \
             VALUES (TRUE, $1) \
             ON CONFLICT (singleton) DO NOTHING \
             RETURNING network",
        )
        .bind(requested)
        .fetch_optional(&mut *transaction)
        .await?;

        let bound = sqlx::query_scalar::<_, String>(
            "SELECT network \
             FROM database_network_binding \
             WHERE singleton = TRUE \
             FOR UPDATE",
        )
        .fetch_one(&mut *transaction)
        .await?;
        let bound_network = parse_network(&bound)?;
        if bound_network != network {
            return Err(PersistenceError::DatabaseNetworkMismatch {
                bound: bound_network,
                requested: network,
            });
        }

        transaction.commit().await?;
        Ok(inserted.is_some())
    }

    pub async fn bound_network(&self) -> Result<Option<Network>, PersistenceError> {
        sqlx::query_scalar::<_, String>(
            "SELECT network \
             FROM database_network_binding \
             WHERE singleton = TRUE",
        )
        .fetch_optional(&self.pool)
        .await?
        .map(|value| parse_network(&value))
        .transpose()
    }
}
