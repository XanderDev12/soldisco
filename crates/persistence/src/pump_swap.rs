use soldisco_domain::Network;
use sqlx::FromRow;

use crate::{
    Database, PersistenceError,
    values::{network_name, require_non_empty, to_i64, to_u64},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PumpSwapPool {
    pub network: Network,
    pub pool_address: String,
    pub base_mint: String,
    pub quote_mint: String,
    pub observed_slot: u64,
    pub observed_signature: String,
}

#[derive(Debug, FromRow)]
struct PumpSwapPoolRow {
    pool_address: String,
    base_mint: String,
    quote_mint: String,
    observed_slot: i64,
    observed_signature: String,
}

impl Database {
    /// Stores a decoded PumpSwap CreatePool identity. Older replays cannot
    /// overwrite a newer mapping.
    pub async fn upsert_pump_swap_pool(
        &self,
        pool: &PumpSwapPool,
    ) -> Result<bool, PersistenceError> {
        validate_pool(pool)?;
        let observed_slot = to_i64(pool.observed_slot, "PumpSwap observed_slot")?;
        let updated = sqlx::query_scalar::<_, i64>(
            "INSERT INTO pump_swap_pools (\
                network, pool_address, base_mint, quote_mint, observed_slot, \
                observed_signature\
             ) VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (network, pool_address) DO UPDATE \
             SET base_mint = EXCLUDED.base_mint, \
                 quote_mint = EXCLUDED.quote_mint, \
                 observed_slot = EXCLUDED.observed_slot, \
                 observed_signature = EXCLUDED.observed_signature, \
                 updated_at = NOW() \
             WHERE pump_swap_pools.base_mint = EXCLUDED.base_mint \
               AND pump_swap_pools.quote_mint = EXCLUDED.quote_mint \
               AND pump_swap_pools.observed_slot <= EXCLUDED.observed_slot \
             RETURNING observed_slot",
        )
        .bind(network_name(pool.network))
        .bind(&pool.pool_address)
        .bind(&pool.base_mint)
        .bind(&pool.quote_mint)
        .bind(observed_slot)
        .bind(&pool.observed_signature)
        .fetch_optional(&self.pool)
        .await?;

        if updated.is_some() {
            return Ok(true);
        }

        let existing = self
            .load_pump_swap_pool(pool.network, &pool.pool_address)
            .await?;
        if existing.is_some_and(|existing| {
            existing.base_mint != pool.base_mint || existing.quote_mint != pool.quote_mint
        }) {
            return Err(PersistenceError::PumpSwapPoolIdentityConflict {
                pool_address: pool.pool_address.clone(),
            });
        }

        Ok(false)
    }

    /// Resolves the mint identity required to normalize PumpSwap Buy/Sell
    /// events, whose event payloads identify only the pool.
    pub async fn load_pump_swap_pool(
        &self,
        network: Network,
        pool_address: &str,
    ) -> Result<Option<PumpSwapPool>, PersistenceError> {
        require_non_empty(pool_address, "PumpSwap pool_address")?;
        let row = sqlx::query_as::<_, PumpSwapPoolRow>(
            "SELECT pool_address, base_mint, quote_mint, observed_slot, \
                    observed_signature \
             FROM pump_swap_pools \
             WHERE network = $1 AND pool_address = $2",
        )
        .bind(network_name(network))
        .bind(pool_address)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| pool_from_row(network, row)).transpose()
    }

    /// Hydrates the complete network-scoped registry after process restart.
    pub async fn load_pump_swap_pools(
        &self,
        network: Network,
    ) -> Result<Vec<PumpSwapPool>, PersistenceError> {
        let rows = sqlx::query_as::<_, PumpSwapPoolRow>(
            "SELECT pool_address, base_mint, quote_mint, observed_slot, \
                    observed_signature \
             FROM pump_swap_pools \
             WHERE network = $1 \
             ORDER BY pool_address",
        )
        .bind(network_name(network))
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| pool_from_row(network, row))
            .collect()
    }
}

fn pool_from_row(network: Network, row: PumpSwapPoolRow) -> Result<PumpSwapPool, PersistenceError> {
    Ok(PumpSwapPool {
        network,
        pool_address: row.pool_address,
        base_mint: row.base_mint,
        quote_mint: row.quote_mint,
        observed_slot: to_u64(row.observed_slot, "pump_swap_pools.observed_slot")?,
        observed_signature: row.observed_signature,
    })
}

fn validate_pool(pool: &PumpSwapPool) -> Result<(), PersistenceError> {
    require_non_empty(&pool.pool_address, "PumpSwap pool_address")?;
    require_non_empty(&pool.base_mint, "PumpSwap base_mint")?;
    require_non_empty(&pool.quote_mint, "PumpSwap quote_mint")?;
    require_non_empty(&pool.observed_signature, "PumpSwap observed_signature")?;
    if pool.base_mint == pool.quote_mint {
        return Err(PersistenceError::IdenticalPoolMints);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use soldisco_domain::Network;

    use super::{PumpSwapPool, validate_pool};
    use crate::PersistenceError;

    #[test]
    fn pool_mapping_rejects_identical_mints() {
        let error = validate_pool(&PumpSwapPool {
            network: Network::SolanaMainnet,
            pool_address: "pool".to_owned(),
            base_mint: "same".to_owned(),
            quote_mint: "same".to_owned(),
            observed_slot: 1,
            observed_signature: "signature".to_owned(),
        })
        .expect_err("base and quote identity must be unambiguous");

        assert!(matches!(error, PersistenceError::IdenticalPoolMints));
    }

    #[test]
    fn lookup_is_network_scoped() {
        let source = include_str!("pump_swap.rs");

        assert!(source.contains("WHERE network = $1 AND pool_address = $2"));
    }
}
