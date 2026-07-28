use soldisco_api_contracts::DiscoveryActivity;
use soldisco_domain::{NormalizedObservation, ObservationPayload, TradeSide};
use sqlx::FromRow;

use crate::{
    PersistenceError,
    values::{network_name, require_non_empty, source_program_name, to_u64, venue_name},
};

#[derive(Debug, FromRow)]
struct ActivityRow {
    trades: i64,
    buys: i64,
    sells: i64,
    unique_traders: i64,
    base_volume_units: String,
    quote_volume_units: String,
}

pub(crate) async fn apply_observation_activity(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    observation: &NormalizedObservation,
) -> Result<DiscoveryActivity, PersistenceError> {
    let network = network_name(observation.key.network);
    let source_program = source_program_name(observation.key.program);
    let venue = venue_name(observation.market.venue);
    let market_address = &observation.market.market_address;
    let mint = &observation.market.mint;
    let quote_mint = observation.market.quote_mint.as_deref().unwrap_or("");
    sqlx::query(
        "INSERT INTO discovery_activity (\
            network, source_program, venue, market_address, mint, quote_mint\
         ) VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (\
            network, source_program, venue, market_address, mint, quote_mint\
         ) DO NOTHING",
    )
    .bind(network)
    .bind(source_program)
    .bind(venue)
    .bind(market_address)
    .bind(mint)
    .bind(quote_mint)
    .execute(&mut **transaction)
    .await?;

    if let ObservationPayload::Trade {
        side,
        wallet,
        base_amount_units,
        quote_amount_units,
        ..
    } = &observation.payload
    {
        require_non_empty(wallet, "trade wallet")?;
        sqlx::query(
            "INSERT INTO discovery_traders (\
                network, source_program, venue, market_address, mint, \
                quote_mint, wallet\
             ) VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (\
                network, source_program, venue, market_address, mint, \
                quote_mint, wallet\
             ) DO NOTHING",
        )
        .bind(network)
        .bind(source_program)
        .bind(venue)
        .bind(market_address)
        .bind(mint)
        .bind(quote_mint)
        .bind(wallet)
        .execute(&mut **transaction)
        .await?;

        let (buy_increment, sell_increment) = match side {
            TradeSide::Buy => (1_i64, 0_i64),
            TradeSide::Sell => (0_i64, 1_i64),
        };
        sqlx::query(
            "UPDATE discovery_activity \
             SET trades = trades + 1, \
                 buys = buys + $2, \
                 sells = sells + $3, \
                 base_volume_units = base_volume_units + $4::NUMERIC, \
                 quote_volume_units = quote_volume_units + $5::NUMERIC, \
                 updated_at = NOW() \
             WHERE network = $6 \
               AND source_program = $7 \
               AND venue = $8 \
               AND market_address = $9 \
               AND mint = $1 \
               AND quote_mint = $10",
        )
        .bind(mint)
        .bind(buy_increment)
        .bind(sell_increment)
        .bind(base_amount_units.to_string())
        .bind(quote_amount_units.to_string())
        .bind(network)
        .bind(source_program)
        .bind(venue)
        .bind(market_address)
        .bind(quote_mint)
        .execute(&mut **transaction)
        .await?;
    }

    let row = sqlx::query_as::<_, ActivityRow>(
        "SELECT activity.trades, activity.buys, activity.sells, \
                (SELECT COUNT(*) FROM discovery_traders AS trader \
                  WHERE trader.network = activity.network \
                    AND trader.source_program = activity.source_program \
                    AND trader.venue = activity.venue \
                    AND trader.market_address = activity.market_address \
                    AND trader.mint = activity.mint \
                    AND trader.quote_mint = activity.quote_mint) \
                    AS unique_traders, \
                activity.base_volume_units::TEXT AS base_volume_units, \
                activity.quote_volume_units::TEXT AS quote_volume_units \
         FROM discovery_activity AS activity \
         WHERE activity.network = $1 \
           AND activity.source_program = $2 \
           AND activity.venue = $3 \
           AND activity.market_address = $4 \
           AND activity.mint = $5 \
           AND activity.quote_mint = $6",
    )
    .bind(network)
    .bind(source_program)
    .bind(venue)
    .bind(market_address)
    .bind(mint)
    .bind(quote_mint)
    .fetch_one(&mut **transaction)
    .await?;

    Ok(DiscoveryActivity {
        trades: to_u64(row.trades, "discovery_activity.trades")?,
        buys: to_u64(row.buys, "discovery_activity.buys")?,
        sells: to_u64(row.sells, "discovery_activity.sells")?,
        unique_traders: to_u64(row.unique_traders, "discovery_activity.unique_traders")?,
        base_volume_units: row.base_volume_units,
        quote_volume_units: row.quote_volume_units,
    })
}
