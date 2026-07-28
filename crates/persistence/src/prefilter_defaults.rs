use soldisco_api_contracts::{PrefilterDefaults, PrefilterDefaultsValidationError};
use sqlx::FromRow;

use crate::{
    Database, PersistenceError,
    values::{to_i64, to_u64},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredPrefilterDefaults {
    pub values: PrefilterDefaults,
    pub revision: u64,
}

#[derive(Debug, FromRow)]
struct PrefilterDefaultsRow {
    revision: i64,
    max_event_age_ms: i64,
    observation_window_ms: i64,
    max_active_windows: i64,
    rpc_requests_per_second: i64,
    rpc_max_in_flight: i64,
    rpc_request_timeout_ms: i64,
    rpc_rate_limit_cooldown_ms: i64,
}

impl Database {
    /// Seeds persisted global prefilter values once.
    ///
    /// Existing values are deliberately left untouched so changing an
    /// environment variable cannot silently overwrite a later UI decision.
    pub async fn initialize_prefilter_defaults(
        &self,
        values: PrefilterDefaults,
    ) -> Result<StoredPrefilterDefaults, PersistenceError> {
        values.validate()?;
        sqlx::query(
            "INSERT INTO prefilter_defaults (\
                singleton, revision, max_event_age_ms, observation_window_ms, \
                max_active_windows, rpc_requests_per_second, rpc_max_in_flight, \
                rpc_request_timeout_ms, rpc_rate_limit_cooldown_ms\
             ) VALUES (TRUE, 1, $1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (singleton) DO NOTHING",
        )
        .bind(to_i64(values.max_event_age_ms, "max_event_age_ms")?)
        .bind(to_i64(
            values.observation_window_ms,
            "observation_window_ms",
        )?)
        .bind(i64::from(values.max_active_windows))
        .bind(i64::from(values.rpc_requests_per_second))
        .bind(i64::from(values.rpc_max_in_flight))
        .bind(to_i64(
            values.rpc_request_timeout_ms,
            "rpc_request_timeout_ms",
        )?)
        .bind(to_i64(
            values.rpc_rate_limit_cooldown_ms,
            "rpc_rate_limit_cooldown_ms",
        )?)
        .execute(&self.pool)
        .await?;

        self.load_prefilter_defaults().await
    }

    pub async fn load_prefilter_defaults(
        &self,
    ) -> Result<StoredPrefilterDefaults, PersistenceError> {
        let row = sqlx::query_as::<_, PrefilterDefaultsRow>(
            "SELECT revision, max_event_age_ms, observation_window_ms, \
                    max_active_windows, rpc_requests_per_second, \
                    rpc_max_in_flight, rpc_request_timeout_ms, \
                    rpc_rate_limit_cooldown_ms \
             FROM prefilter_defaults \
             WHERE singleton = TRUE",
        )
        .fetch_one(&self.pool)
        .await?;

        row.try_into()
    }

    /// Replaces the complete prefilter configuration under an optimistic
    /// revision check. Partial updates are intentionally unsupported so one
    /// saved revision always describes a coherent collector configuration.
    pub async fn update_prefilter_defaults(
        &self,
        expected_revision: u64,
        values: PrefilterDefaults,
    ) -> Result<StoredPrefilterDefaults, PersistenceError> {
        if expected_revision == 0 {
            return Err(PersistenceError::MustBePositive {
                field: "prefilter expected_revision",
            });
        }
        values.validate()?;
        let expected_revision = to_i64(expected_revision, "prefilter expected_revision")?;

        let updated = sqlx::query_as::<_, PrefilterDefaultsRow>(
            "UPDATE prefilter_defaults \
             SET revision = revision + 1, \
                 max_event_age_ms = $2, \
                 observation_window_ms = $3, \
                 max_active_windows = $4, \
                 rpc_requests_per_second = $5, \
                 rpc_max_in_flight = $6, \
                 rpc_request_timeout_ms = $7, \
                 rpc_rate_limit_cooldown_ms = $8, \
                 updated_at = NOW() \
             WHERE singleton = TRUE AND revision = $1 \
             RETURNING revision, max_event_age_ms, observation_window_ms, \
                       max_active_windows, rpc_requests_per_second, \
                       rpc_max_in_flight, rpc_request_timeout_ms, \
                       rpc_rate_limit_cooldown_ms",
        )
        .bind(expected_revision)
        .bind(to_i64(values.max_event_age_ms, "max_event_age_ms")?)
        .bind(to_i64(
            values.observation_window_ms,
            "observation_window_ms",
        )?)
        .bind(i64::from(values.max_active_windows))
        .bind(i64::from(values.rpc_requests_per_second))
        .bind(i64::from(values.rpc_max_in_flight))
        .bind(to_i64(
            values.rpc_request_timeout_ms,
            "rpc_request_timeout_ms",
        )?)
        .bind(to_i64(
            values.rpc_rate_limit_cooldown_ms,
            "rpc_rate_limit_cooldown_ms",
        )?)
        .fetch_optional(&self.pool)
        .await?;

        match updated {
            Some(row) => row.try_into(),
            None => {
                let actual = sqlx::query_scalar::<_, i64>(
                    "SELECT revision FROM prefilter_defaults WHERE singleton = TRUE",
                )
                .fetch_optional(&self.pool)
                .await?
                .map(|revision| to_u64(revision, "prefilter_defaults.revision"))
                .transpose()?;
                Err(PersistenceError::PrefilterDefaultsRevisionConflict {
                    expected: to_u64(expected_revision, "prefilter expected_revision")?,
                    actual,
                })
            }
        }
    }
}

impl TryFrom<PrefilterDefaultsRow> for StoredPrefilterDefaults {
    type Error = PersistenceError;

    fn try_from(row: PrefilterDefaultsRow) -> Result<Self, Self::Error> {
        let values = PrefilterDefaults {
            max_event_age_ms: to_u64(row.max_event_age_ms, "prefilter_defaults.max_event_age_ms")?,
            observation_window_ms: to_u64(
                row.observation_window_ms,
                "prefilter_defaults.observation_window_ms",
            )?,
            max_active_windows: to_u32(
                row.max_active_windows,
                "prefilter_defaults.max_active_windows",
            )?,
            rpc_requests_per_second: to_u32(
                row.rpc_requests_per_second,
                "prefilter_defaults.rpc_requests_per_second",
            )?,
            rpc_max_in_flight: to_u32(
                row.rpc_max_in_flight,
                "prefilter_defaults.rpc_max_in_flight",
            )?,
            rpc_request_timeout_ms: to_u64(
                row.rpc_request_timeout_ms,
                "prefilter_defaults.rpc_request_timeout_ms",
            )?,
            rpc_rate_limit_cooldown_ms: to_u64(
                row.rpc_rate_limit_cooldown_ms,
                "prefilter_defaults.rpc_rate_limit_cooldown_ms",
            )?,
        };
        values.validate().map_err(invalid_prefilter_defaults)?;
        Ok(Self {
            values,
            revision: to_u64(row.revision, "prefilter_defaults.revision")?,
        })
    }
}

fn to_u32(value: i64, field: &'static str) -> Result<u32, PersistenceError> {
    u32::try_from(value).map_err(|_| PersistenceError::InvalidStoredValue {
        field,
        value: value.to_string(),
    })
}

fn invalid_prefilter_defaults(error: PrefilterDefaultsValidationError) -> PersistenceError {
    PersistenceError::InvalidPrefilterDefaults(error)
}

#[cfg(test)]
mod tests {
    use soldisco_api_contracts::{PrefilterDefaults, PrefilterDefaultsValidationError};

    use super::{PrefilterDefaultsRow, StoredPrefilterDefaults};
    use crate::PersistenceError;

    #[test]
    fn invalid_stored_settings_fail_closed() {
        let row = PrefilterDefaultsRow {
            revision: 1,
            max_event_age_ms: 15_000,
            observation_window_ms: 60_000,
            max_active_windows: 128,
            rpc_requests_per_second: 1,
            rpc_max_in_flight: 0,
            rpc_request_timeout_ms: 5_000,
            rpc_rate_limit_cooldown_ms: 5_000,
        };

        assert!(matches!(
            StoredPrefilterDefaults::try_from(row),
            Err(PersistenceError::InvalidPrefilterDefaults(
                PrefilterDefaultsValidationError::OutOfBounds {
                    field: "rpc_max_in_flight",
                    ..
                }
            ))
        ));
    }

    #[test]
    fn valid_stored_settings_preserve_exact_values() {
        let expected = PrefilterDefaults {
            max_event_age_ms: 15_000,
            observation_window_ms: 60_000,
            max_active_windows: 128,
            rpc_requests_per_second: 1,
            rpc_max_in_flight: 4,
            rpc_request_timeout_ms: 5_000,
            rpc_rate_limit_cooldown_ms: 5_000,
        };
        let row = PrefilterDefaultsRow {
            revision: 9,
            max_event_age_ms: 15_000,
            observation_window_ms: 60_000,
            max_active_windows: 128,
            rpc_requests_per_second: 1,
            rpc_max_in_flight: 4,
            rpc_request_timeout_ms: 5_000,
            rpc_rate_limit_cooldown_ms: 5_000,
        };

        assert_eq!(
            StoredPrefilterDefaults::try_from(row).expect("valid stored settings"),
            StoredPrefilterDefaults {
                values: expected,
                revision: 9,
            }
        );
    }
}
