use base64::{Engine as _, engine::general_purpose::STANDARD};
use soldisco_domain::{ChainCoordinate, ObservationKey};
use sqlx::FromRow;

use crate::{
    Database, PersistenceError,
    values::{
        network_name, parse_network, parse_source_program, require_non_empty, source_program_name,
        to_i64, to_u64,
    },
};

pub const MAX_QUARANTINE_EVIDENCE_BASE64_BYTES: usize = 16_384;
const MAX_DECODER_VERSION_BYTES: usize = 128;
const MAX_REASON_CODE_BYTES: usize = 128;
const MAX_REASON_DETAIL_BYTES: usize = 2_048;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntakeQuarantineRecord {
    pub key: ObservationKey,
    pub decoder_version: String,
    pub reason_code: String,
    /// Credential-free diagnostic context. Callers must never place RPC URLs,
    /// API keys, authorization headers, or wallet secrets here.
    pub reason_detail: String,
    pub evidence_base64: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredIntakeQuarantine {
    pub record: IntakeQuarantineRecord,
    pub occurrences: u64,
    pub first_seen_unix_ms: i64,
    pub last_seen_unix_ms: i64,
}

#[derive(Debug, FromRow)]
struct StoredQuarantineRow {
    network: String,
    source_program: String,
    slot: i64,
    transaction_index: Option<i64>,
    signature: String,
    instruction_index: i32,
    event_index: i32,
    decoder_version: String,
    reason_code: String,
    reason_detail: String,
    evidence_base64: String,
    occurrences: i64,
    first_seen_unix_ms: i64,
    last_seen_unix_ms: i64,
}

impl Database {
    /// Records a structurally invalid or unsupported source event without
    /// admitting it to `chain_observations`. Replays of the exact chain
    /// coordinate increment `occurrences` instead of growing without bound.
    pub async fn record_intake_quarantine(
        &self,
        record: &IntakeQuarantineRecord,
    ) -> Result<u64, PersistenceError> {
        validate_record(record)?;
        let coordinate = &record.key.coordinate;
        let occurrences = sqlx::query_scalar::<_, i64>(
            "INSERT INTO intake_quarantine (\
                network, source_program, slot, transaction_index, signature, \
                instruction_index, event_index, decoder_version, reason_code, \
                reason_detail, evidence_base64\
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) \
             ON CONFLICT (\
                network, source_program, slot, signature, instruction_index, \
                event_index\
             ) DO UPDATE \
             SET transaction_index = COALESCE(\
                    EXCLUDED.transaction_index, \
                    intake_quarantine.transaction_index\
                 ), \
                 decoder_version = EXCLUDED.decoder_version, \
                 reason_code = EXCLUDED.reason_code, \
                 reason_detail = EXCLUDED.reason_detail, \
                 evidence_base64 = EXCLUDED.evidence_base64, \
                 occurrences = intake_quarantine.occurrences + 1, \
                 last_seen_at = NOW() \
             RETURNING occurrences",
        )
        .bind(network_name(record.key.network))
        .bind(source_program_name(record.key.program))
        .bind(to_i64(coordinate.slot, "quarantine slot")?)
        .bind(
            coordinate
                .transaction_index
                .map(|value| to_i64(value, "quarantine transaction_index"))
                .transpose()?,
        )
        .bind(&coordinate.signature)
        .bind(i32::from(coordinate.instruction_index))
        .bind(i32::from(coordinate.event_index))
        .bind(&record.decoder_version)
        .bind(&record.reason_code)
        .bind(&record.reason_detail)
        .bind(&record.evidence_base64)
        .fetch_one(&self.pool)
        .await?;

        to_u64(occurrences, "intake_quarantine.occurrences")
    }

    pub async fn load_intake_quarantine(
        &self,
        key: &ObservationKey,
    ) -> Result<Option<StoredIntakeQuarantine>, PersistenceError> {
        require_non_empty(&key.coordinate.signature, "quarantine signature")?;
        let row = sqlx::query_as::<_, StoredQuarantineRow>(
            "SELECT network, source_program, slot, transaction_index, \
                    signature, instruction_index, event_index, decoder_version, \
                    reason_code, reason_detail, evidence_base64, occurrences, \
                    (EXTRACT(EPOCH FROM first_seen_at) * 1000)::BIGINT \
                        AS first_seen_unix_ms, \
                    (EXTRACT(EPOCH FROM last_seen_at) * 1000)::BIGINT \
                        AS last_seen_unix_ms \
             FROM intake_quarantine \
             WHERE network = $1 \
               AND source_program = $2 \
               AND slot = $3 \
               AND signature = $4 \
               AND instruction_index = $5 \
               AND event_index = $6",
        )
        .bind(network_name(key.network))
        .bind(source_program_name(key.program))
        .bind(to_i64(key.coordinate.slot, "quarantine slot")?)
        .bind(&key.coordinate.signature)
        .bind(i32::from(key.coordinate.instruction_index))
        .bind(i32::from(key.coordinate.event_index))
        .fetch_optional(&self.pool)
        .await?;

        row.map(stored_record).transpose()
    }
}

fn validate_record(record: &IntakeQuarantineRecord) -> Result<(), PersistenceError> {
    require_non_empty(&record.key.coordinate.signature, "quarantine signature")?;
    require_non_empty(&record.decoder_version, "decoder_version")?;
    require_non_empty(&record.reason_code, "reason_code")?;
    require_non_empty(&record.evidence_base64, "evidence_base64")?;
    validate_length(
        &record.decoder_version,
        "decoder_version",
        MAX_DECODER_VERSION_BYTES,
    )?;
    validate_length(&record.reason_code, "reason_code", MAX_REASON_CODE_BYTES)?;
    validate_length(
        &record.reason_detail,
        "reason_detail",
        MAX_REASON_DETAIL_BYTES,
    )?;
    validate_length(
        &record.evidence_base64,
        "evidence_base64",
        MAX_QUARANTINE_EVIDENCE_BASE64_BYTES,
    )?;
    validate_evidence_base64(&record.evidence_base64, "evidence_base64")?;
    Ok(())
}

pub(crate) fn validate_evidence_base64(
    value: &str,
    field: &'static str,
) -> Result<(), PersistenceError> {
    require_non_empty(value, field)?;
    validate_length(value, field, MAX_QUARANTINE_EVIDENCE_BASE64_BYTES)?;
    STANDARD
        .decode(value)
        .map_err(|_| PersistenceError::InvalidBase64 { field })?;
    Ok(())
}

pub(crate) fn validate_length(
    value: &str,
    field: &'static str,
    maximum: usize,
) -> Result<(), PersistenceError> {
    if value.len() > maximum {
        Err(PersistenceError::FieldTooLong { field, maximum })
    } else {
        Ok(())
    }
}

fn stored_record(row: StoredQuarantineRow) -> Result<StoredIntakeQuarantine, PersistenceError> {
    let network = parse_network(&row.network)?;
    let program = parse_source_program(&row.source_program)?;
    Ok(StoredIntakeQuarantine {
        record: IntakeQuarantineRecord {
            key: ObservationKey {
                network,
                program,
                coordinate: ChainCoordinate {
                    slot: to_u64(row.slot, "intake_quarantine.slot")?,
                    transaction_index: row
                        .transaction_index
                        .map(|value| to_u64(value, "intake_quarantine.transaction_index"))
                        .transpose()?,
                    signature: row.signature,
                    instruction_index: u16::try_from(row.instruction_index).map_err(|_| {
                        PersistenceError::InvalidStoredValue {
                            field: "intake_quarantine.instruction_index",
                            value: row.instruction_index.to_string(),
                        }
                    })?,
                    event_index: u16::try_from(row.event_index).map_err(|_| {
                        PersistenceError::InvalidStoredValue {
                            field: "intake_quarantine.event_index",
                            value: row.event_index.to_string(),
                        }
                    })?,
                },
            },
            decoder_version: row.decoder_version,
            reason_code: row.reason_code,
            reason_detail: row.reason_detail,
            evidence_base64: row.evidence_base64,
        },
        occurrences: to_u64(row.occurrences, "intake_quarantine.occurrences")?,
        first_seen_unix_ms: row.first_seen_unix_ms,
        last_seen_unix_ms: row.last_seen_unix_ms,
    })
}

#[cfg(test)]
mod tests {
    use soldisco_domain::{ChainCoordinate, Network, ObservationKey, SourceProgram};

    use super::{IntakeQuarantineRecord, validate_record};
    use crate::PersistenceError;

    fn record(evidence_base64: &str) -> IntakeQuarantineRecord {
        IntakeQuarantineRecord {
            key: ObservationKey {
                network: Network::SolanaMainnet,
                program: SourceProgram::Pump,
                coordinate: ChainCoordinate {
                    slot: 1,
                    transaction_index: None,
                    signature: "signature".to_owned(),
                    instruction_index: 0,
                    event_index: 0,
                },
            },
            decoder_version: "pump-v1".to_owned(),
            reason_code: "MALFORMED_EVENT".to_owned(),
            reason_detail: "event fields did not match the published IDL".to_owned(),
            evidence_base64: evidence_base64.to_owned(),
        }
    }

    #[test]
    fn rejects_non_base64_evidence() {
        assert!(matches!(
            validate_record(&record("***")),
            Err(PersistenceError::InvalidBase64 {
                field: "evidence_base64"
            })
        ));
    }
}
