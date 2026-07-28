use std::time::Duration;

use serde_json::{Value, json};
use soldisco_domain::NormalizedObservation;
use sqlx::{FromRow, Postgres, Transaction};

use crate::{
    Database, PersistenceError,
    values::{duration_millis, require_non_empty},
};

const MAX_CLAIM_BATCH: u32 = 1_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaimedObservationWork {
    pub work_id: i64,
    pub observation_id: i64,
    pub work_kind: String,
    pub attempts: u32,
    pub lease_owner: String,
    pub lease_expires_at_unix_ms: i64,
    pub observation: NormalizedObservation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkFailureDisposition {
    RetryScheduled,
    PermanentlyFailed,
}

#[derive(Debug, FromRow)]
struct ClaimedWorkRow {
    work_id: i64,
    observation_id: i64,
    work_kind: String,
    attempts: i32,
    lease_owner: String,
    lease_expires_at_unix_ms: i64,
    normalized_observation: Value,
}

#[derive(Debug)]
pub(crate) struct OwnedWork {
    pub(crate) work_id: i64,
    pub(crate) observation_id: i64,
    pub(crate) work_kind: String,
    pub(crate) attempts: i32,
    pub(crate) observation: NormalizedObservation,
}

#[derive(Debug, FromRow)]
struct OwnedWorkRow {
    work_id: i64,
    observation_id: i64,
    work_kind: String,
    attempts: i32,
    observation: Value,
}

impl Database {
    /// Claims available work with a time-bounded lease.
    ///
    /// `FOR UPDATE SKIP LOCKED` lets multiple workers claim independent rows
    /// without waiting on each other. Expired (and legacy lease-less)
    /// processing rows are recoverable after a worker crash.
    pub async fn claim_observation_work(
        &self,
        work_kind: &str,
        worker_id: &str,
        limit: u32,
        lease_duration: Duration,
    ) -> Result<Vec<ClaimedObservationWork>, PersistenceError> {
        require_non_empty(work_kind, "work_kind")?;
        require_non_empty(worker_id, "worker_id")?;
        if limit == 0 {
            return Err(PersistenceError::MustBePositive {
                field: "claim limit",
            });
        }
        if limit > MAX_CLAIM_BATCH {
            return Err(PersistenceError::LimitTooLarge {
                field: "claim limit",
                maximum: MAX_CLAIM_BATCH,
            });
        }
        let lease_millis = duration_millis(lease_duration, "lease_duration", false)?;

        let rows = sqlx::query_as::<_, ClaimedWorkRow>(
            "WITH claimable AS (\
                SELECT id \
                FROM observation_work \
                WHERE work_kind = $1 \
                  AND attempts < 2147483647 \
                  AND (\
                    (status = 'PENDING' AND available_at <= NOW()) \
                    OR \
                    (status = 'PROCESSING' AND (\
                        lease_expires_at IS NULL OR lease_expires_at <= NOW()\
                    ))\
                  ) \
                ORDER BY available_at, id \
                LIMIT $2 \
                FOR UPDATE SKIP LOCKED\
             ), claimed AS (\
                UPDATE observation_work AS work \
                SET status = 'PROCESSING', \
                    attempts = work.attempts + 1, \
                    claimed_at = NOW(), \
                    finished_at = NULL, \
                    lease_owner = $3, \
                    lease_expires_at = NOW() \
                        + ($4::DOUBLE PRECISION * INTERVAL '1 millisecond'), \
                    updated_at = NOW() \
                FROM claimable \
                WHERE work.id = claimable.id \
                RETURNING work.id, work.observation_id, work.work_kind, \
                          work.attempts, work.lease_owner, work.lease_expires_at\
             ) \
             SELECT claimed.id AS work_id, claimed.observation_id, \
                    claimed.work_kind, claimed.attempts, claimed.lease_owner, \
                    (EXTRACT(EPOCH FROM claimed.lease_expires_at) * 1000)::BIGINT \
                        AS lease_expires_at_unix_ms, \
                    observation.normalized_observation \
             FROM claimed \
             JOIN chain_observations AS observation \
               ON observation.id = claimed.observation_id \
             ORDER BY claimed.id",
        )
        .bind(work_kind)
        .bind(i64::from(limit))
        .bind(worker_id)
        .bind(lease_millis)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(ClaimedObservationWork {
                    work_id: row.work_id,
                    observation_id: row.observation_id,
                    work_kind: row.work_kind,
                    attempts: u32::try_from(row.attempts).map_err(|_| {
                        PersistenceError::InvalidStoredValue {
                            field: "observation_work.attempts",
                            value: row.attempts.to_string(),
                        }
                    })?,
                    lease_owner: row.lease_owner,
                    lease_expires_at_unix_ms: row.lease_expires_at_unix_ms,
                    observation: serde_json::from_value(row.normalized_observation)?,
                })
            })
            .collect()
    }

    pub async fn renew_observation_work_lease(
        &self,
        work_id: i64,
        worker_id: &str,
        lease_duration: Duration,
    ) -> Result<i64, PersistenceError> {
        require_non_empty(worker_id, "worker_id")?;
        let lease_millis = duration_millis(lease_duration, "lease_duration", false)?;
        let renewed_until = sqlx::query_scalar::<_, i64>(
            "UPDATE observation_work \
             SET lease_expires_at = NOW() \
                    + ($3::DOUBLE PRECISION * INTERVAL '1 millisecond'), \
                 updated_at = NOW() \
             WHERE id = $1 \
               AND status = 'PROCESSING' \
               AND lease_owner = $2 \
               AND lease_expires_at > NOW() \
             RETURNING (EXTRACT(EPOCH FROM lease_expires_at) * 1000)::BIGINT",
        )
        .bind(work_id)
        .bind(worker_id)
        .bind(lease_millis)
        .fetch_optional(&self.pool)
        .await?;

        renewed_until.ok_or_else(|| PersistenceError::WorkLeaseLost {
            work_id,
            worker_id: worker_id.to_owned(),
        })
    }

    /// Completes a non-discovery work item.
    ///
    /// Discovery work must use `commit_discovery_observation`,
    /// `commit_discovery_approval`, or `commit_discovery_rejection` so the
    /// durable projection and work transition cannot diverge.
    pub async fn complete_observation_work(
        &self,
        work_id: i64,
        worker_id: &str,
    ) -> Result<(), PersistenceError> {
        require_non_empty(worker_id, "worker_id")?;
        let mut transaction = self.pool.begin().await?;
        let work = lock_owned_work(&mut transaction, work_id, worker_id).await?;
        if work.work_kind == "DISCOVERY" {
            return Err(PersistenceError::DiscoveryOutcomeRequired { work_id });
        }
        mark_work_complete(&mut transaction, work_id).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Releases failed work for retry, or permanently fails it when the
    /// attempt budget has been exhausted.
    pub async fn retry_or_fail_observation_work(
        &self,
        work_id: i64,
        worker_id: &str,
        error_code: &str,
        retry_after: Duration,
        max_attempts: u32,
    ) -> Result<WorkFailureDisposition, PersistenceError> {
        require_non_empty(worker_id, "worker_id")?;
        require_non_empty(error_code, "error_code")?;
        if max_attempts == 0 {
            return Err(PersistenceError::MustBePositive {
                field: "max_attempts",
            });
        }
        let max_attempts =
            i32::try_from(max_attempts).map_err(|_| PersistenceError::ValueOutOfRange {
                field: "max_attempts",
            })?;
        let retry_millis = duration_millis(retry_after, "retry_after", true)?;

        let mut transaction = self.pool.begin().await?;
        let work = lock_owned_work(&mut transaction, work_id, worker_id).await?;
        let permanently_failed = work.attempts >= max_attempts;

        sqlx::query(
            "UPDATE observation_work \
             SET status = CASE WHEN $4 THEN 'FAILED' ELSE 'PENDING' END, \
                 available_at = CASE \
                    WHEN $4 THEN available_at \
                    ELSE NOW() + ($3::DOUBLE PRECISION * INTERVAL '1 millisecond') \
                 END, \
                 claimed_at = CASE WHEN $4 THEN claimed_at ELSE NULL END, \
                 finished_at = CASE WHEN $4 THEN NOW() ELSE NULL END, \
                 last_error_code = $2, \
                 lease_owner = NULL, \
                 lease_expires_at = NULL, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(work_id)
        .bind(error_code)
        .bind(retry_millis)
        .bind(permanently_failed)
        .execute(&mut *transaction)
        .await?;

        let disposition = if permanently_failed {
            if work.work_kind == "DISCOVERY" {
                record_terminal_discovery_failure(&mut transaction, &work, error_code).await?;
            }
            WorkFailureDisposition::PermanentlyFailed
        } else {
            WorkFailureDisposition::RetryScheduled
        };

        transaction.commit().await?;
        Ok(disposition)
    }
}

pub(crate) async fn lock_owned_work(
    transaction: &mut Transaction<'_, Postgres>,
    work_id: i64,
    worker_id: &str,
) -> Result<OwnedWork, PersistenceError> {
    let row = sqlx::query_as::<_, OwnedWorkRow>(
        "SELECT work.id AS work_id, work.observation_id, work.work_kind, \
                work.attempts, observation.normalized_observation AS observation \
         FROM observation_work AS work \
         JOIN chain_observations AS observation \
           ON observation.id = work.observation_id \
         WHERE work.id = $1 \
           AND work.status = 'PROCESSING' \
           AND work.lease_owner = $2 \
           AND work.lease_expires_at > NOW() \
         FOR UPDATE OF work",
    )
    .bind(work_id)
    .bind(worker_id)
    .fetch_optional(&mut **transaction)
    .await?;

    row.map(|row| {
        Ok::<OwnedWork, PersistenceError>(OwnedWork {
            work_id: row.work_id,
            observation_id: row.observation_id,
            work_kind: row.work_kind,
            attempts: row.attempts,
            observation: serde_json::from_value(row.observation)?,
        })
    })
    .transpose()?
    .ok_or_else(|| PersistenceError::WorkLeaseLost {
        work_id,
        worker_id: worker_id.to_owned(),
    })
}

pub(crate) async fn mark_work_complete(
    transaction: &mut Transaction<'_, Postgres>,
    work_id: i64,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "UPDATE observation_work \
         SET status = 'COMPLETE', finished_at = NOW(), \
             lease_owner = NULL, lease_expires_at = NULL, updated_at = NOW() \
         WHERE id = $1",
    )
    .bind(work_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn record_terminal_discovery_failure(
    transaction: &mut Transaction<'_, Postgres>,
    work: &OwnedWork,
    error_code: &str,
) -> Result<(), PersistenceError> {
    let sequence = sqlx::query_scalar::<_, i64>(
        "UPDATE discovery_projection_state \
         SET pending = GREATEST(pending - 1, 0), \
             rejected = rejected + 1, \
             sequence = sequence + 1, \
             updated_at = NOW() \
         WHERE singleton = TRUE \
         RETURNING sequence",
    )
    .fetch_one(&mut **transaction)
    .await?;

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
    .bind(error_code)
    .bind(work.observation.received_time_unix_ms)
    .execute(&mut **transaction)
    .await?;

    sqlx::query(
        "INSERT INTO projection_events (projection_name, event_kind, payload) \
         VALUES ('DISCOVERY', 'DISCOVERY_WORK_FAILED', $1)",
    )
    .bind(json!({
        "projection_sequence": sequence,
        "work_id": work.work_id,
        "observation_id": work.observation_id,
        "mint": work.observation.market.mint,
        "reason_code": error_code,
    }))
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        PersistenceError,
        values::{duration_millis, require_non_empty},
    };

    #[test]
    fn lease_duration_must_be_positive() {
        let error = duration_millis(Duration::ZERO, "lease_duration", false)
            .expect_err("zero lease can never safely own work");

        assert!(matches!(
            error,
            PersistenceError::MustBePositive {
                field: "lease_duration"
            }
        ));
    }

    #[test]
    fn worker_identity_must_not_be_whitespace() {
        assert!(matches!(
            require_non_empty("  ", "worker_id"),
            Err(PersistenceError::EmptyField { field: "worker_id" })
        ));
    }

    #[test]
    fn work_claim_uses_skip_locked_and_expired_lease_recovery() {
        let source = include_str!("work.rs");

        assert!(source.contains("FOR UPDATE SKIP LOCKED"));
        assert!(source.contains("lease_expires_at <= NOW()"));
        assert!(source.contains("status = 'PROCESSING'"));
    }
}
