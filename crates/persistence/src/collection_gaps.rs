use soldisco_domain::{Network, SourceProgram};
use sqlx::FromRow;

use crate::{
    Database, PersistenceError,
    quarantine::validate_length,
    values::{
        network_name, parse_network, parse_source_program, require_non_empty, source_program_name,
        to_i64, to_u64,
    },
};

const MAX_REASON_CODE_BYTES: usize = 128;
const MAX_DETAILS_BYTES: usize = 2_048;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionPosition {
    pub slot: u64,
    pub transaction_index: Option<u64>,
    pub signature: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewCollectionGap {
    pub network: Network,
    pub source_program: SourceProgram,
    pub reason_code: String,
    pub prior_checkpoint: CollectionPosition,
    pub detected_at_unix_ms: i64,
    /// Credential-free explanation only. Never include RPC URLs, API keys,
    /// authorization headers, signing material, or wallet secrets.
    pub details: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CollectionGap {
    pub gap_id: i64,
    pub network: Network,
    pub source_program: SourceProgram,
    pub reason_code: String,
    pub prior_checkpoint: CollectionPosition,
    pub detected_at_unix_ms: i64,
    pub details: String,
    pub resolved_through: Option<CollectionPosition>,
    pub resolved_at_unix_ms: Option<i64>,
}

#[derive(Debug, FromRow)]
struct CollectionGapRow {
    gap_id: i64,
    network: String,
    source_program: String,
    reason_code: String,
    prior_checkpoint_slot: i64,
    prior_checkpoint_transaction_index: Option<i64>,
    prior_checkpoint_signature: String,
    detected_at_unix_ms: i64,
    details: String,
    resolved_through_slot: Option<i64>,
    resolved_through_transaction_index: Option<i64>,
    resolved_through_signature: Option<String>,
    resolved_at_unix_ms: Option<i64>,
}

impl Database {
    /// Appends (or deduplicates) an active gap when bounded recovery cannot
    /// reach the durable checkpoint. The returned id is stable for retries of
    /// the same unresolved gap.
    pub async fn record_collection_gap(
        &self,
        gap: &NewCollectionGap,
    ) -> Result<i64, PersistenceError> {
        validate_new_gap(gap)?;
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO collection_gaps (\
                network, source_program, reason_code, prior_checkpoint_slot, \
                prior_checkpoint_transaction_index, prior_checkpoint_signature, \
                detected_at, details\
             ) VALUES (\
                $1, $2, $3, $4, $5, $6, \
                TIMESTAMPTZ 'epoch' + ($7 * INTERVAL '1 millisecond'), $8\
             ) \
             ON CONFLICT (\
                network, source_program, reason_code, prior_checkpoint_slot, \
                prior_checkpoint_signature\
             ) WHERE resolved_through_slot IS NULL \
             DO UPDATE \
             SET prior_checkpoint_transaction_index = COALESCE(\
                    collection_gaps.prior_checkpoint_transaction_index, \
                    EXCLUDED.prior_checkpoint_transaction_index\
                 ), \
                 detected_at = LEAST(\
                    collection_gaps.detected_at, \
                    EXCLUDED.detected_at\
                 ), \
                 details = EXCLUDED.details \
             RETURNING id",
        )
        .bind(network_name(gap.network))
        .bind(source_program_name(gap.source_program))
        .bind(&gap.reason_code)
        .bind(to_i64(gap.prior_checkpoint.slot, "prior checkpoint slot")?)
        .bind(
            gap.prior_checkpoint
                .transaction_index
                .map(|value| to_i64(value, "prior checkpoint transaction_index"))
                .transpose()?,
        )
        .bind(&gap.prior_checkpoint.signature)
        .bind(gap.detected_at_unix_ms)
        .bind(&gap.details)
        .fetch_one(&self.pool)
        .await
        .map_err(PersistenceError::from)
    }

    /// Marks an active gap recovered through an explicit chain position.
    /// Returns `false` when the gap does not exist or was already resolved.
    pub async fn resolve_collection_gap(
        &self,
        gap_id: i64,
        resolved_through: &CollectionPosition,
    ) -> Result<bool, PersistenceError> {
        validate_position(resolved_through, "resolved through signature")?;
        let mut transaction = self.pool.begin().await?;
        let prior = sqlx::query_as::<_, (i64, Option<i64>)>(
            "SELECT prior_checkpoint_slot, prior_checkpoint_transaction_index \
             FROM collection_gaps \
             WHERE id = $1 AND resolved_through_slot IS NULL \
             FOR UPDATE",
        )
        .bind(gap_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some((prior_slot, prior_index)) = prior else {
            transaction.commit().await?;
            return Ok(false);
        };
        if position_precedes(
            resolved_through,
            to_u64(prior_slot, "collection_gaps.prior_checkpoint_slot")?,
            prior_index
                .map(|value| to_u64(value, "collection_gaps.prior_checkpoint_transaction_index"))
                .transpose()?,
        ) {
            return Err(PersistenceError::InvalidGapResolution);
        }

        let result = sqlx::query(
            "UPDATE collection_gaps \
             SET resolved_through_slot = $2, \
                 resolved_through_transaction_index = $3, \
                 resolved_through_signature = $4, \
                 resolved_at = NOW() \
             WHERE id = $1 AND resolved_through_slot IS NULL",
        )
        .bind(gap_id)
        .bind(to_i64(resolved_through.slot, "resolved through slot")?)
        .bind(
            resolved_through
                .transaction_index
                .map(|value| to_i64(value, "resolved through transaction_index"))
                .transpose()?,
        )
        .bind(&resolved_through.signature)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn load_active_collection_gaps(
        &self,
        network: Network,
        source_program: SourceProgram,
    ) -> Result<Vec<CollectionGap>, PersistenceError> {
        sqlx::query_as::<_, CollectionGapRow>(
            "SELECT id AS gap_id, network, source_program, reason_code, \
                    prior_checkpoint_slot, prior_checkpoint_transaction_index, \
                    prior_checkpoint_signature, \
                    (EXTRACT(EPOCH FROM detected_at) * 1000)::BIGINT \
                        AS detected_at_unix_ms, \
                    details, resolved_through_slot, \
                    resolved_through_transaction_index, \
                    resolved_through_signature, \
                    CASE WHEN resolved_at IS NULL THEN NULL \
                         ELSE (EXTRACT(EPOCH FROM resolved_at) * 1000)::BIGINT \
                    END AS resolved_at_unix_ms \
             FROM collection_gaps \
             WHERE network = $1 \
               AND source_program = $2 \
               AND resolved_through_slot IS NULL \
             ORDER BY detected_at, id",
        )
        .bind(network_name(network))
        .bind(source_program_name(source_program))
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(map_gap)
        .collect()
    }

    pub async fn count_active_collection_gaps(
        &self,
        network: Network,
        source_program: SourceProgram,
    ) -> Result<u64, PersistenceError> {
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) \
             FROM collection_gaps \
             WHERE network = $1 \
               AND source_program = $2 \
               AND resolved_through_slot IS NULL",
        )
        .bind(network_name(network))
        .bind(source_program_name(source_program))
        .fetch_one(&self.pool)
        .await?;
        to_u64(count, "collection_gaps.active_count")
    }
}

fn validate_new_gap(gap: &NewCollectionGap) -> Result<(), PersistenceError> {
    require_non_empty(&gap.reason_code, "collection gap reason_code")?;
    require_non_empty(&gap.details, "collection gap details")?;
    validate_length(
        &gap.reason_code,
        "collection gap reason_code",
        MAX_REASON_CODE_BYTES,
    )?;
    validate_length(&gap.details, "collection gap details", MAX_DETAILS_BYTES)?;
    validate_position(
        &gap.prior_checkpoint,
        "collection gap prior checkpoint signature",
    )
}

fn validate_position(
    position: &CollectionPosition,
    signature_field: &'static str,
) -> Result<(), PersistenceError> {
    require_non_empty(&position.signature, signature_field)
}

fn position_precedes(
    position: &CollectionPosition,
    prior_slot: u64,
    prior_transaction_index: Option<u64>,
) -> bool {
    position.slot < prior_slot
        || (position.slot == prior_slot
            && matches!(
                (position.transaction_index, prior_transaction_index),
                (Some(position_index), Some(prior_index))
                    if position_index < prior_index
            ))
}

fn map_gap(row: CollectionGapRow) -> Result<CollectionGap, PersistenceError> {
    let resolved_through = match (
        row.resolved_through_slot,
        row.resolved_through_transaction_index,
        row.resolved_through_signature,
    ) {
        (None, None, None) => None,
        (Some(slot), transaction_index, Some(signature)) => Some(CollectionPosition {
            slot: to_u64(slot, "collection_gaps.resolved_through_slot")?,
            transaction_index: transaction_index
                .map(|value| to_u64(value, "collection_gaps.resolved_through_transaction_index"))
                .transpose()?,
            signature,
        }),
        values => {
            return Err(PersistenceError::InvalidStoredValue {
                field: "collection_gaps.resolution",
                value: format!("{values:?}"),
            });
        }
    };
    Ok(CollectionGap {
        gap_id: row.gap_id,
        network: parse_network(&row.network)?,
        source_program: parse_source_program(&row.source_program)?,
        reason_code: row.reason_code,
        prior_checkpoint: CollectionPosition {
            slot: to_u64(
                row.prior_checkpoint_slot,
                "collection_gaps.prior_checkpoint_slot",
            )?,
            transaction_index: row
                .prior_checkpoint_transaction_index
                .map(|value| to_u64(value, "collection_gaps.prior_checkpoint_transaction_index"))
                .transpose()?,
            signature: row.prior_checkpoint_signature,
        },
        detected_at_unix_ms: row.detected_at_unix_ms,
        details: row.details,
        resolved_through,
        resolved_at_unix_ms: row.resolved_at_unix_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::{CollectionPosition, position_precedes};

    #[test]
    fn same_slot_only_orders_when_both_indexes_exist() {
        let without_index = CollectionPosition {
            slot: 10,
            transaction_index: None,
            signature: "signature".to_owned(),
        };
        assert!(!position_precedes(&without_index, 10, Some(4)));

        let indexed = CollectionPosition {
            transaction_index: Some(3),
            ..without_index
        };
        assert!(position_precedes(&indexed, 10, Some(4)));
        assert!(!position_precedes(&indexed, 10, None));
    }
}
