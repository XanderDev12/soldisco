use soldisco_api_contracts::{QualificationDefaults, QualificationDefaultsValidationError};
use sqlx::FromRow;

use crate::{
    Database, PersistenceError,
    values::{to_i64, to_u64},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoredQualificationDefaults {
    pub values: QualificationDefaults,
    pub revision: u64,
}

#[derive(Debug, FromRow)]
struct QualificationDefaultsRow {
    revision: i64,
    minimum_trades: i64,
    minimum_unique_traders: i64,
    minimum_buys: i64,
    minimum_sells: i64,
    minimum_native_quote_volume_units: String,
    minimum_stable_quote_volume_units: String,
    maximum_single_wallet_quote_share_bps: i32,
}

impl Database {
    /// Seeds the append-only qualification history once. Existing operator
    /// choices always win over later process defaults.
    pub async fn initialize_qualification_defaults(
        &self,
        values: QualificationDefaults,
    ) -> Result<StoredQualificationDefaults, PersistenceError> {
        values.validate()?;
        let mut transaction = self.pool.begin().await?;
        sqlx::query("LOCK TABLE qualification_defaults_current IN SHARE ROW EXCLUSIVE MODE")
            .execute(&mut *transaction)
            .await?;
        let current_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(\
                SELECT 1 FROM qualification_defaults_current WHERE singleton = TRUE\
             )",
        )
        .fetch_one(&mut *transaction)
        .await?;
        if !current_exists {
            let revision = insert_revision(&mut transaction, values).await?;
            sqlx::query(
                "INSERT INTO qualification_defaults_current (singleton, revision) \
                 VALUES (TRUE, $1)",
            )
            .bind(revision)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;

        self.load_qualification_defaults().await
    }

    pub async fn load_qualification_defaults(
        &self,
    ) -> Result<StoredQualificationDefaults, PersistenceError> {
        load_current(&self.pool).await
    }

    /// Appends a complete coherent settings revision under an optimistic lock.
    /// Open windows keep their pinned prior revision and value snapshot.
    pub async fn update_qualification_defaults(
        &self,
        expected_revision: u64,
        values: QualificationDefaults,
    ) -> Result<StoredQualificationDefaults, PersistenceError> {
        if expected_revision == 0 {
            return Err(PersistenceError::MustBePositive {
                field: "qualification expected_revision",
            });
        }
        values.validate()?;
        let expected = to_i64(expected_revision, "qualification expected_revision")?;
        let mut transaction = self.pool.begin().await?;
        let actual = sqlx::query_scalar::<_, i64>(
            "SELECT revision \
             FROM qualification_defaults_current \
             WHERE singleton = TRUE \
             FOR UPDATE",
        )
        .fetch_optional(&mut *transaction)
        .await?;
        if actual != Some(expected) {
            transaction.rollback().await?;
            return Err(PersistenceError::QualificationDefaultsRevisionConflict {
                expected: expected_revision,
                actual: actual
                    .map(|revision| to_u64(revision, "qualification_defaults_current.revision"))
                    .transpose()?,
            });
        }

        let revision = insert_revision(&mut transaction, values).await?;
        sqlx::query(
            "UPDATE qualification_defaults_current \
             SET revision = $1 \
             WHERE singleton = TRUE",
        )
        .bind(revision)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;

        self.load_qualification_defaults().await
    }
}

async fn insert_revision(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    values: QualificationDefaults,
) -> Result<i64, PersistenceError> {
    Ok(sqlx::query_scalar::<_, i64>(
        "INSERT INTO qualification_defaults_revisions (\
            minimum_trades, minimum_unique_traders, minimum_buys, \
            minimum_sells, minimum_native_quote_volume_units, \
            minimum_stable_quote_volume_units, \
            maximum_single_wallet_quote_share_bps\
         ) VALUES ($1, $2, $3, $4, $5::NUMERIC, $6::NUMERIC, $7) \
         RETURNING revision",
    )
    .bind(i64::from(values.minimum_trades))
    .bind(i64::from(values.minimum_unique_traders))
    .bind(i64::from(values.minimum_buys))
    .bind(i64::from(values.minimum_sells))
    .bind(values.minimum_native_quote_volume_units.to_string())
    .bind(values.minimum_stable_quote_volume_units.to_string())
    .bind(i32::from(values.maximum_single_wallet_quote_share_bps))
    .fetch_one(&mut **transaction)
    .await?)
}

async fn load_current<'e, E>(executor: E) -> Result<StoredQualificationDefaults, PersistenceError>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row = sqlx::query_as::<_, QualificationDefaultsRow>(
        "SELECT revision.revision, revision.minimum_trades, \
                revision.minimum_unique_traders, revision.minimum_buys, \
                revision.minimum_sells, \
                revision.minimum_native_quote_volume_units::TEXT \
                    AS minimum_native_quote_volume_units, \
                revision.minimum_stable_quote_volume_units::TEXT \
                    AS minimum_stable_quote_volume_units, \
                revision.maximum_single_wallet_quote_share_bps \
         FROM qualification_defaults_current AS current \
         JOIN qualification_defaults_revisions AS revision \
           ON revision.revision = current.revision \
         WHERE current.singleton = TRUE",
    )
    .fetch_one(executor)
    .await?;
    row.try_into()
}

impl TryFrom<QualificationDefaultsRow> for StoredQualificationDefaults {
    type Error = PersistenceError;

    fn try_from(row: QualificationDefaultsRow) -> Result<Self, Self::Error> {
        let values = QualificationDefaults {
            minimum_trades: to_u32(row.minimum_trades, "minimum_trades")?,
            minimum_unique_traders: to_u32(row.minimum_unique_traders, "minimum_unique_traders")?,
            minimum_buys: to_u32(row.minimum_buys, "minimum_buys")?,
            minimum_sells: to_u32(row.minimum_sells, "minimum_sells")?,
            minimum_native_quote_volume_units: parse_u64_decimal(
                &row.minimum_native_quote_volume_units,
                "minimum_native_quote_volume_units",
            )?,
            minimum_stable_quote_volume_units: parse_u64_decimal(
                &row.minimum_stable_quote_volume_units,
                "minimum_stable_quote_volume_units",
            )?,
            maximum_single_wallet_quote_share_bps: u16::try_from(
                row.maximum_single_wallet_quote_share_bps,
            )
            .map_err(|_| PersistenceError::InvalidStoredValue {
                field: "maximum_single_wallet_quote_share_bps",
                value: row.maximum_single_wallet_quote_share_bps.to_string(),
            })?,
        };
        values.validate().map_err(invalid_qualification_defaults)?;
        Ok(Self {
            values,
            revision: to_u64(row.revision, "qualification_defaults.revision")?,
        })
    }
}

fn to_u32(value: i64, field: &'static str) -> Result<u32, PersistenceError> {
    u32::try_from(value).map_err(|_| PersistenceError::InvalidStoredValue {
        field,
        value: value.to_string(),
    })
}

fn parse_u64_decimal(value: &str, field: &'static str) -> Result<u64, PersistenceError> {
    value
        .parse::<u64>()
        .map_err(|_| PersistenceError::InvalidStoredValue {
            field,
            value: value.to_owned(),
        })
}

fn invalid_qualification_defaults(error: QualificationDefaultsValidationError) -> PersistenceError {
    PersistenceError::InvalidQualificationDefaults(error)
}
