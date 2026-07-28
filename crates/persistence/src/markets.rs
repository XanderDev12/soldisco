use soldisco_domain::{MarketIdentity, Network};
use sqlx::FromRow;

use crate::{
    Database, PersistenceError,
    values::{network_name, parse_venue},
};

#[derive(Debug, FromRow)]
struct MarketRow {
    mint: String,
    venue: String,
    market_address: String,
    quote_mint: Option<String>,
}

impl Database {
    /// Loads retained market identities from explicit relational columns for
    /// collector registry hydration. This does not materialize or depend on
    /// the browser-facing projection JSON.
    pub async fn load_discovery_markets(
        &self,
        network: Network,
    ) -> Result<Vec<MarketIdentity>, PersistenceError> {
        sqlx::query_as::<_, MarketRow>(
            "SELECT mint, venue, market_address, quote_mint \
             FROM discovery_tokens \
             WHERE network = $1 \
               AND NOT (\
                    venue = 'PUMP_BONDING_CURVE' \
                    AND (\
                        event_kind IN ('COMPLETE', 'COMPLETE_PUMP_AMM_MIGRATION') \
                        OR EXISTS (\
                            SELECT 1 FROM chain_observations AS terminal \
                            WHERE terminal.network = discovery_tokens.network \
                              AND terminal.source_program = 'PUMP' \
                              AND terminal.mint = discovery_tokens.mint \
                              AND terminal.event_kind IN (\
                                  'COMPLETE', 'COMPLETE_PUMP_AMM_MIGRATION'\
                              )\
                        )\
                    )\
               ) \
             ORDER BY mint",
        )
        .bind(network_name(network))
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(|row| {
            Ok(MarketIdentity {
                network,
                mint: row.mint,
                venue: parse_venue(&row.venue)?,
                market_address: row.market_address,
                quote_mint: row.quote_mint,
            })
        })
        .collect()
    }
}
