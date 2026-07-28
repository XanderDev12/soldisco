use crate::{Database, PersistenceError, values::to_u64};

pub const MAX_RETENTION_BATCH_SIZE: u32 = 10_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetentionPolicy {
    pub terminal_history_before_unix_ms: i64,
    pub projection_events_before_unix_ms: i64,
    pub quarantine_before_unix_ms: i64,
    pub batch_size: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RetentionPruneResult {
    pub terminal_observations: u64,
    pub projection_events: u64,
    pub quarantine_records: u64,
}

impl RetentionPruneResult {
    #[must_use]
    pub const fn total_rows(self) -> u64 {
        self.terminal_observations
            .saturating_add(self.projection_events)
            .saturating_add(self.quarantine_records)
    }
}

impl Database {
    pub async fn database_size_bytes(&self) -> Result<u64, PersistenceError> {
        let bytes =
            sqlx::query_scalar::<_, i64>("SELECT pg_database_size(current_database())::BIGINT")
                .fetch_one(&self.pool)
                .await?;
        to_u64(bytes, "pg_database_size")
    }

    /// Prunes one bounded batch from each raw-history class. Discovery token,
    /// activity, counter, rejection-summary, checkpoint, pool, and gap
    /// aggregates are intentionally never touched.
    pub async fn prune_retained_history(
        &self,
        policy: RetentionPolicy,
    ) -> Result<RetentionPruneResult, PersistenceError> {
        validate_policy(policy)?;
        let mut transaction = self.pool.begin().await?;
        let terminal_observations = sqlx::query(
            "WITH candidates AS (\
                SELECT observation.id \
                FROM chain_observations AS observation \
                WHERE observation.first_seen_at \
                        < TIMESTAMPTZ 'epoch' \
                          + ($1 * INTERVAL '1 millisecond') \
                  AND EXISTS (\
                    SELECT 1 \
                    FROM observation_work AS terminal_work \
                    WHERE terminal_work.observation_id = observation.id\
                  ) \
                  AND NOT EXISTS (\
                    SELECT 1 \
                    FROM observation_work AS retained_work \
                    WHERE retained_work.observation_id = observation.id \
                      AND (\
                        retained_work.status NOT IN ('COMPLETE', 'FAILED') \
                        OR retained_work.finished_at IS NULL \
                        OR retained_work.finished_at \
                            >= TIMESTAMPTZ 'epoch' \
                               + ($1 * INTERVAL '1 millisecond')\
                      )\
                  ) \
                  AND NOT EXISTS (\
                    SELECT 1 \
                    FROM discovery_window_observations AS member \
                    JOIN discovery_windows AS active_window \
                      ON active_window.id = member.window_id \
                    WHERE member.observation_id = observation.id \
                      AND active_window.status = 'ACTIVE'\
                  ) \
                ORDER BY observation.first_seen_at, observation.id \
                LIMIT $2 \
                FOR UPDATE OF observation SKIP LOCKED\
             ) \
             DELETE FROM chain_observations AS observation \
             USING candidates \
             WHERE observation.id = candidates.id",
        )
        .bind(policy.terminal_history_before_unix_ms)
        .bind(i64::from(policy.batch_size))
        .execute(&mut *transaction)
        .await?
        .rows_affected();

        let projection_events = sqlx::query(
            "WITH candidates AS (\
                SELECT sequence \
                FROM projection_events \
                WHERE created_at \
                        < TIMESTAMPTZ 'epoch' \
                          + ($1 * INTERVAL '1 millisecond') \
                ORDER BY created_at, sequence \
                LIMIT $2 \
                FOR UPDATE SKIP LOCKED\
             ) \
             DELETE FROM projection_events AS event \
             USING candidates \
             WHERE event.sequence = candidates.sequence",
        )
        .bind(policy.projection_events_before_unix_ms)
        .bind(i64::from(policy.batch_size))
        .execute(&mut *transaction)
        .await?
        .rows_affected();

        let quarantine_records = sqlx::query(
            "WITH candidates AS (\
                SELECT id \
                FROM intake_quarantine \
                WHERE last_seen_at \
                        < TIMESTAMPTZ 'epoch' \
                          + ($1 * INTERVAL '1 millisecond') \
                ORDER BY last_seen_at, id \
                LIMIT $2 \
                FOR UPDATE SKIP LOCKED\
             ) \
             DELETE FROM intake_quarantine AS quarantine \
             USING candidates \
             WHERE quarantine.id = candidates.id",
        )
        .bind(policy.quarantine_before_unix_ms)
        .bind(i64::from(policy.batch_size))
        .execute(&mut *transaction)
        .await?
        .rows_affected();

        transaction.commit().await?;
        Ok(RetentionPruneResult {
            terminal_observations,
            projection_events,
            quarantine_records,
        })
    }
}

fn validate_policy(policy: RetentionPolicy) -> Result<(), PersistenceError> {
    if policy.batch_size == 0 {
        return Err(PersistenceError::MustBePositive {
            field: "retention batch_size",
        });
    }
    if policy.batch_size > MAX_RETENTION_BATCH_SIZE {
        return Err(PersistenceError::LimitTooLarge {
            field: "retention batch_size",
            maximum: MAX_RETENTION_BATCH_SIZE,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MAX_RETENTION_BATCH_SIZE, RetentionPolicy, validate_policy};
    use crate::PersistenceError;

    #[test]
    fn retention_batch_is_bounded() {
        let policy = RetentionPolicy {
            terminal_history_before_unix_ms: 0,
            projection_events_before_unix_ms: 0,
            quarantine_before_unix_ms: 0,
            batch_size: MAX_RETENTION_BATCH_SIZE + 1,
        };
        assert!(matches!(
            validate_policy(policy),
            Err(PersistenceError::LimitTooLarge {
                field: "retention batch_size",
                ..
            })
        ));
    }
}
