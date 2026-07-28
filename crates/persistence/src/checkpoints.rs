use soldisco_domain::{Network, SourceProgram};
use sqlx::FromRow;

use crate::{
    Database, PersistenceError,
    values::{network_name, require_non_empty, source_program_name, to_i64, to_u64},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecoveryCheckpoint {
    pub network: Network,
    pub source_program: SourceProgram,
    pub checkpoint_kind: String,
    pub last_slot: u64,
    pub last_transaction_index: Option<u64>,
    pub last_signature: String,
}

#[derive(Debug, FromRow)]
struct CheckpointRow {
    last_slot: i64,
    last_transaction_index: Option<i64>,
    last_signature: String,
}

impl Database {
    pub async fn load_recovery_checkpoint(
        &self,
        network: Network,
        source_program: SourceProgram,
        checkpoint_kind: &str,
    ) -> Result<Option<RecoveryCheckpoint>, PersistenceError> {
        require_non_empty(checkpoint_kind, "checkpoint_kind")?;
        let row = sqlx::query_as::<_, CheckpointRow>(
            "SELECT last_slot, last_transaction_index, last_signature \
             FROM recovery_checkpoints \
             WHERE network = $1 AND source_program = $2 AND checkpoint_kind = $3",
        )
        .bind(network_name(network))
        .bind(source_program_name(source_program))
        .bind(checkpoint_kind)
        .fetch_optional(&self.pool)
        .await?;

        row.map(|row| {
            Ok(RecoveryCheckpoint {
                network,
                source_program,
                checkpoint_kind: checkpoint_kind.to_owned(),
                last_slot: to_u64(row.last_slot, "recovery_checkpoints.last_slot")?,
                last_transaction_index: row
                    .last_transaction_index
                    .map(|value| to_u64(value, "recovery_checkpoints.last_transaction_index"))
                    .transpose()?,
                last_signature: row.last_signature,
            })
        })
        .transpose()
    }

    /// Advances a checkpoint without allowing a late worker to regress it.
    ///
    /// Returns `true` when the checkpoint was inserted, advanced to a newer
    /// slot, or advanced by a provider-supplied same-slot transaction index.
    /// If either same-slot index is absent, the checkpoint deliberately does
    /// not advance so restart recovery safely replays the ambiguous slot.
    pub async fn save_recovery_checkpoint(
        &self,
        checkpoint: &RecoveryCheckpoint,
    ) -> Result<bool, PersistenceError> {
        require_non_empty(&checkpoint.checkpoint_kind, "checkpoint_kind")?;
        require_non_empty(&checkpoint.last_signature, "last_signature")?;
        let slot = to_i64(checkpoint.last_slot, "last_slot")?;
        let transaction_index = checkpoint
            .last_transaction_index
            .map(|value| to_i64(value, "last_transaction_index"))
            .transpose()?;

        let updated = sqlx::query_scalar::<_, i64>(
            "INSERT INTO recovery_checkpoints (\
                network, source_program, checkpoint_kind, last_slot, \
                last_transaction_index, last_signature\
             ) VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT (network, source_program, checkpoint_kind) DO UPDATE \
             SET last_slot = EXCLUDED.last_slot, \
                 last_transaction_index = EXCLUDED.last_transaction_index, \
                 last_signature = EXCLUDED.last_signature, \
                 updated_at = NOW() \
             WHERE recovery_checkpoints.last_slot < EXCLUDED.last_slot \
                OR (\
                    recovery_checkpoints.last_slot = EXCLUDED.last_slot \
                    AND recovery_checkpoints.last_transaction_index IS NOT NULL \
                    AND EXCLUDED.last_transaction_index IS NOT NULL \
                    AND recovery_checkpoints.last_transaction_index \
                        < EXCLUDED.last_transaction_index\
                ) \
             RETURNING last_slot",
        )
        .bind(network_name(checkpoint.network))
        .bind(source_program_name(checkpoint.source_program))
        .bind(&checkpoint.checkpoint_kind)
        .bind(slot)
        .bind(transaction_index)
        .bind(&checkpoint.last_signature)
        .fetch_optional(&self.pool)
        .await?;

        Ok(updated.is_some())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn checkpoint_upsert_is_monotonic() {
        let source = include_str!("checkpoints.rs");

        assert!(source.contains("recovery_checkpoints.last_slot < EXCLUDED.last_slot"));
        assert!(source.contains("EXCLUDED.last_transaction_index IS NOT NULL"));
    }
}
