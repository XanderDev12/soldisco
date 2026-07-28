use serde_json::Value;
use soldisco_api_contracts::DiscoveryToken;
use soldisco_domain::NormalizedObservation;

use crate::{
    PersistenceError,
    values::{discovery_stage_name, network_name, source_program_name, to_i64, venue_name},
};

pub(crate) async fn insert_discovery_token(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    observation: &NormalizedObservation,
    token: &DiscoveryToken,
    work_id: i64,
    validation_source: &str,
    validation_version: &str,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO discovery_tokens (\
            mint, stage, network, source_program, venue, market_address, \
            quote_mint, event_kind, observed_slot, observed_signature, \
            instruction_index, event_index, decision_source, decision_version, \
            token, source_work_id\
         ) VALUES (\
            $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, \
            $15, $16\
         )",
    )
    .bind(&token.mint)
    .bind(discovery_stage_name(token.stage))
    .bind(network_name(observation.key.network))
    .bind(source_program_name(token.source_program))
    .bind(venue_name(token.primary_venue))
    .bind(&token.market_address)
    .bind(&token.quote_mint)
    .bind(&token.last_event_kind)
    .bind(to_i64(token.observed_slot, "observed_slot")?)
    .bind(&token.latest_signature)
    .bind(i32::from(observation.key.coordinate.instruction_index))
    .bind(i32::from(observation.key.coordinate.event_index))
    .bind(validation_source)
    .bind(validation_version)
    .bind(serde_json::to_value(token)?)
    .bind(work_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(crate) async fn update_discovery_token(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    observation: &NormalizedObservation,
    token: &DiscoveryToken,
    work_id: i64,
    validation_source: &str,
    validation_version: &str,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "UPDATE discovery_tokens \
         SET stage = $2, network = $3, source_program = $4, venue = $5, \
             market_address = $6, quote_mint = $7, event_kind = $8, \
             observed_slot = $9, observed_signature = $10, \
             instruction_index = $11, event_index = $12, \
             decision_source = $13, decision_version = $14, token = $15, \
             source_work_id = $16, updated_at = NOW() \
         WHERE mint = $1",
    )
    .bind(&token.mint)
    .bind(discovery_stage_name(token.stage))
    .bind(network_name(observation.key.network))
    .bind(source_program_name(token.source_program))
    .bind(venue_name(token.primary_venue))
    .bind(&token.market_address)
    .bind(&token.quote_mint)
    .bind(&token.last_event_kind)
    .bind(to_i64(token.observed_slot, "observed_slot")?)
    .bind(&token.latest_signature)
    .bind(i32::from(observation.key.coordinate.instruction_index))
    .bind(i32::from(observation.key.coordinate.event_index))
    .bind(validation_source)
    .bind(validation_version)
    .bind(serde_json::to_value(token)?)
    .bind(work_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Enriches descriptive metadata without replacing the newer relational
/// market identity, chain coordinate, decision provenance, or activity.
pub(crate) async fn update_discovery_token_metadata(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    token: &DiscoveryToken,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "UPDATE discovery_tokens \
         SET token = $2, updated_at = NOW() \
         WHERE mint = $1",
    )
    .bind(&token.mint)
    .bind(serde_json::to_value(token)?)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(crate) async fn upsert_rejection_summary(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    reason_code: &str,
    seen_at_unix_ms: i64,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO discovery_rejection_summaries (\
            reason_code, count, last_seen_unix_ms\
         ) VALUES ($1, 1, $2) \
         ON CONFLICT (reason_code) DO UPDATE \
         SET count = discovery_rejection_summaries.count + 1, \
             last_seen_unix_ms = GREATEST(\
                discovery_rejection_summaries.last_seen_unix_ms, \
                EXCLUDED.last_seen_unix_ms\
             ), \
             updated_at = NOW()",
    )
    .bind(reason_code)
    .bind(seen_at_unix_ms)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

pub(crate) async fn append_projection_event(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_kind: &str,
    payload: Value,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO projection_events (projection_name, event_kind, payload) \
         VALUES ('DISCOVERY', $1, $2)",
    )
    .bind(event_kind)
    .bind(payload)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}
