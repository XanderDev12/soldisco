use serde_json::{Value, json};
use soldisco_api_contracts::{
    DiscoveryCounters, DiscoveryMode, DiscoverySnapshot, DiscoveryStage, DiscoveryToken,
    RejectionSummary,
};
use sqlx::FromRow;

use crate::{
    Database, PersistenceError,
    discovery_activity::apply_observation_activity,
    discovery_storage::{
        append_projection_event, insert_discovery_token, update_discovery_token,
        update_discovery_token_metadata, upsert_rejection_summary,
    },
    discovery_validation::{
        enrich_metadata_from_stale_creation, ensure_discovery_work, merge_approval,
        merge_observation, mutation_name, token_matches_observation_market,
        validate_promotion_identity, validate_stored_stage, validate_token_shape,
        validate_token_source,
    },
    values::{
        discovery_mode_name, discovery_stage_name, parse_discovery_mode, require_non_empty, to_i64,
        to_u64,
    },
    work::{lock_owned_work, mark_work_complete},
};

pub const DEFAULT_DISCOVERY_SNAPSHOT_LIMIT: u32 = 500;
pub const MAX_DISCOVERY_SNAPSHOT_LIMIT: u32 = 5_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryObservation {
    pub token: DiscoveryToken,
    /// Identifies the structural validator that admitted the candidate.
    pub validation_source: String,
    /// Version of that validator/decoder contract.
    pub validation_version: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryRejection {
    pub mint: String,
    pub reason_code: String,
    pub seen_at_unix_ms: i64,
    pub decision_source: String,
    pub decision_version: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryProjectionMutation {
    Inserted,
    Updated,
    DemotedByMarketChange,
    StaleObservationIgnored,
    MetadataEnrichedFromStaleCreation,
    Promoted,
    AlreadyApproved,
}

#[derive(Debug, FromRow)]
struct StoredTokenRow {
    stage: String,
    observed_slot: i64,
    token: Value,
}

#[derive(Debug, FromRow)]
struct ProjectionStateRow {
    sequence: i64,
    mode: String,
    observed: i64,
    pending: i64,
    approved: i64,
    rejected: i64,
    flow_per_minute: Option<i64>,
}

#[derive(Debug, FromRow)]
struct RejectionRow {
    reason_code: String,
    count: i64,
    last_seen_unix_ms: i64,
}

impl Database {
    /// Completes discovery work by recording a structurally valid candidate as
    /// `OBSERVED`. It never manufactures an approval or score.
    pub async fn commit_discovery_observation(
        &self,
        work_id: i64,
        worker_id: &str,
        candidate: &DiscoveryObservation,
    ) -> Result<DiscoveryProjectionMutation, PersistenceError> {
        require_non_empty(worker_id, "worker_id")?;
        require_non_empty(&candidate.validation_source, "validation_source")?;
        require_non_empty(&candidate.validation_version, "validation_version")?;
        validate_token_shape(&candidate.token)?;
        if candidate.token.stage != DiscoveryStage::Observed {
            return Err(PersistenceError::ObservationMustBeUnapproved);
        }

        let mut transaction = self.pool.begin().await?;
        let work = lock_owned_work(&mut transaction, work_id, worker_id).await?;
        ensure_discovery_work(&work.work_kind, work_id)?;
        validate_token_source(&candidate.token, &work.observation)?;

        let existing = sqlx::query_as::<_, StoredTokenRow>(
            "SELECT stage, observed_slot, token \
             FROM discovery_tokens \
             WHERE mint = $1 \
             FOR UPDATE",
        )
        .bind(&candidate.token.mint)
        .fetch_optional(&mut *transaction)
        .await?;

        let (mutation, mut token_to_store, is_new, approval_removed) = match existing {
            None => (
                DiscoveryProjectionMutation::Inserted,
                candidate.token.clone(),
                true,
                false,
            ),
            Some(existing) => {
                let stored: DiscoveryToken = serde_json::from_value(existing.token)?;
                validate_stored_stage(&existing.stage, stored.stage)?;
                if existing.observed_slot > to_i64(candidate.token.observed_slot, "observed_slot")?
                {
                    let mut enriched = stored;
                    let changed = enrich_metadata_from_stale_creation(
                        &mut enriched,
                        &candidate.token,
                        &work.observation,
                    );
                    (
                        if changed {
                            DiscoveryProjectionMutation::MetadataEnrichedFromStaleCreation
                        } else {
                            DiscoveryProjectionMutation::StaleObservationIgnored
                        },
                        enriched,
                        false,
                        false,
                    )
                } else {
                    let was_approved = stored.stage == DiscoveryStage::Approved;
                    let merged = merge_observation(stored, candidate.token.clone());
                    let approval_removed = was_approved && merged.stage != DiscoveryStage::Approved;
                    (
                        if approval_removed {
                            DiscoveryProjectionMutation::DemotedByMarketChange
                        } else {
                            DiscoveryProjectionMutation::Updated
                        },
                        merged,
                        false,
                        approval_removed,
                    )
                }
            }
        };

        if is_new {
            insert_discovery_token(
                &mut transaction,
                &work.observation,
                &token_to_store,
                work_id,
                &candidate.validation_source,
                &candidate.validation_version,
            )
            .await?;
        } else if matches!(
            mutation,
            DiscoveryProjectionMutation::Updated
                | DiscoveryProjectionMutation::DemotedByMarketChange
        ) {
            update_discovery_token(
                &mut transaction,
                &work.observation,
                &token_to_store,
                work_id,
                &candidate.validation_source,
                &candidate.validation_version,
            )
            .await?;
        } else if mutation == DiscoveryProjectionMutation::MetadataEnrichedFromStaleCreation {
            update_discovery_token_metadata(&mut transaction, &token_to_store).await?;
        }

        let activity = apply_observation_activity(&mut transaction, &work.observation).await?;
        if token_matches_observation_market(&token_to_store, &work.observation) {
            token_to_store.activity = activity;
            sqlx::query(
                "UPDATE discovery_tokens \
                 SET token = $2, updated_at = NOW() \
                 WHERE mint = $1",
            )
            .bind(&token_to_store.mint)
            .bind(serde_json::to_value(&token_to_store)?)
            .execute(&mut *transaction)
            .await?;
        }

        let sequence = sqlx::query_scalar::<_, i64>(
            "UPDATE discovery_projection_state \
             SET observed = observed + $1, \
                 pending = GREATEST(pending - 1, 0), \
                 approved = GREATEST(approved - $2, 0), \
                 sequence = sequence + 1, \
                 updated_at = NOW() \
             WHERE singleton = TRUE \
             RETURNING sequence",
        )
        .bind(if is_new { 1_i64 } else { 0_i64 })
        .bind(if approval_removed { 1_i64 } else { 0_i64 })
        .fetch_one(&mut *transaction)
        .await?;

        mark_work_complete(&mut transaction, work_id).await?;
        append_projection_event(
            &mut transaction,
            "DISCOVERY_TOKEN_OBSERVED",
            json!({
                "projection_sequence": sequence,
                "work_id": work_id,
                "mint": candidate.token.mint,
                "mutation": mutation_name(mutation),
            }),
        )
        .await?;
        transaction.commit().await?;

        Ok(mutation)
    }

    /// Promotes an existing candidate. This path is deliberately separate from
    /// observation and requires a caller-supplied decision source and version.
    pub async fn commit_discovery_approval(
        &self,
        token: &DiscoveryToken,
        decision_source: &str,
        decision_version: &str,
    ) -> Result<DiscoveryProjectionMutation, PersistenceError> {
        require_non_empty(decision_source, "decision_source")?;
        require_non_empty(decision_version, "decision_version")?;
        validate_token_shape(token)?;
        if token.stage != DiscoveryStage::Approved {
            return Err(PersistenceError::ApprovalMustBeExplicit);
        }

        let mut transaction = self.pool.begin().await?;
        let existing = sqlx::query_as::<_, StoredTokenRow>(
            "SELECT stage, observed_slot, token \
             FROM discovery_tokens \
             WHERE mint = $1 \
             FOR UPDATE",
        )
        .bind(&token.mint)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(|| PersistenceError::CandidateNotObserved {
            mint: token.mint.clone(),
        })?;

        let stored: DiscoveryToken = serde_json::from_value(existing.token)?;
        validate_stored_stage(&existing.stage, stored.stage)?;
        validate_promotion_identity(&stored, token)?;
        let was_approved = stored.stage == DiscoveryStage::Approved;
        let promoted = merge_approval(stored, token.clone());
        let token_json = serde_json::to_value(&promoted)?;

        sqlx::query(
            "UPDATE discovery_tokens \
             SET stage = 'APPROVED', \
                 observed_slot = $2, \
                 observed_signature = $3, \
                 event_kind = $4, \
                 decision_source = $5, \
                 decision_version = $6, \
                 token = $7, \
                 updated_at = NOW() \
             WHERE mint = $1",
        )
        .bind(&promoted.mint)
        .bind(to_i64(promoted.observed_slot, "observed_slot")?)
        .bind(&promoted.latest_signature)
        .bind(&promoted.last_event_kind)
        .bind(decision_source)
        .bind(decision_version)
        .bind(token_json)
        .execute(&mut *transaction)
        .await?;

        let sequence = sqlx::query_scalar::<_, i64>(
            "UPDATE discovery_projection_state \
             SET approved = approved + $1, \
                 sequence = sequence + 1, \
                 updated_at = NOW() \
             WHERE singleton = TRUE \
             RETURNING sequence",
        )
        .bind(if was_approved { 0_i64 } else { 1_i64 })
        .fetch_one(&mut *transaction)
        .await?;

        let mutation = if was_approved {
            DiscoveryProjectionMutation::AlreadyApproved
        } else {
            DiscoveryProjectionMutation::Promoted
        };
        append_projection_event(
            &mut transaction,
            "DISCOVERY_TOKEN_APPROVED",
            json!({
                "projection_sequence": sequence,
                "mint": promoted.mint,
                "mutation": mutation_name(mutation),
                "decision_source": decision_source,
                "decision_version": decision_version,
            }),
        )
        .await?;
        transaction.commit().await?;

        Ok(mutation)
    }

    pub async fn commit_discovery_rejection(
        &self,
        work_id: i64,
        worker_id: &str,
        rejection: &DiscoveryRejection,
    ) -> Result<(), PersistenceError> {
        require_non_empty(worker_id, "worker_id")?;
        require_non_empty(&rejection.mint, "rejection mint")?;
        require_non_empty(&rejection.reason_code, "reason_code")?;
        require_non_empty(&rejection.decision_source, "decision_source")?;
        require_non_empty(&rejection.decision_version, "decision_version")?;

        let mut transaction = self.pool.begin().await?;
        let work = lock_owned_work(&mut transaction, work_id, worker_id).await?;
        ensure_discovery_work(&work.work_kind, work_id)?;
        if rejection.mint != work.observation.market.mint {
            return Err(PersistenceError::DiscoverySourceMismatch { field: "mint" });
        }

        let sequence = sqlx::query_scalar::<_, i64>(
            "UPDATE discovery_projection_state \
             SET pending = GREATEST(pending - 1, 0), \
                 rejected = rejected + 1, \
                 sequence = sequence + 1, \
                 updated_at = NOW() \
             WHERE singleton = TRUE \
             RETURNING sequence",
        )
        .fetch_one(&mut *transaction)
        .await?;
        upsert_rejection_summary(
            &mut transaction,
            &rejection.reason_code,
            rejection.seen_at_unix_ms,
        )
        .await?;
        mark_work_complete(&mut transaction, work_id).await?;
        append_projection_event(
            &mut transaction,
            "DISCOVERY_TOKEN_REJECTED",
            json!({
                "projection_sequence": sequence,
                "work_id": work_id,
                "mint": rejection.mint,
                "reason_code": rejection.reason_code,
                "decision_source": rejection.decision_source,
                "decision_version": rejection.decision_version,
            }),
        )
        .await?;
        transaction.commit().await?;

        Ok(())
    }

    pub async fn load_discovery_snapshot(&self) -> Result<DiscoverySnapshot, PersistenceError> {
        self.load_discovery_snapshot_bounded(DEFAULT_DISCOVERY_SNAPSHOT_LIMIT)
            .await
    }

    pub async fn load_discovery_snapshot_bounded(
        &self,
        limit: u32,
    ) -> Result<DiscoverySnapshot, PersistenceError> {
        if limit == 0 {
            return Err(PersistenceError::MustBePositive {
                field: "discovery snapshot limit",
            });
        }
        if limit > MAX_DISCOVERY_SNAPSHOT_LIMIT {
            return Err(PersistenceError::LimitTooLarge {
                field: "discovery snapshot limit",
                maximum: MAX_DISCOVERY_SNAPSHOT_LIMIT,
            });
        }
        let mut transaction = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *transaction)
            .await?;

        let state = sqlx::query_as::<_, ProjectionStateRow>(
            "SELECT sequence, mode, observed, pending, approved, rejected, \
                    flow_per_minute \
             FROM discovery_projection_state \
             WHERE singleton = TRUE",
        )
        .fetch_one(&mut *transaction)
        .await?;
        let mode = parse_discovery_mode(&state.mode)?;
        let tokens_total = match mode {
            DiscoveryMode::ObserveAll => state.observed,
            DiscoveryMode::ApprovedOnly => state.approved,
        };
        let token_rows = sqlx::query_scalar::<_, Value>(
            "SELECT token \
             FROM discovery_tokens \
             WHERE $1 = 'OBSERVE_ALL' OR stage = 'APPROVED' \
             ORDER BY observed_slot DESC, mint \
             LIMIT $2",
        )
        .bind(discovery_mode_name(mode))
        .bind(i64::from(limit))
        .fetch_all(&mut *transaction)
        .await?;
        let rejection_rows = sqlx::query_as::<_, RejectionRow>(
            "SELECT reason_code, count, last_seen_unix_ms \
             FROM discovery_rejection_summaries \
             ORDER BY count DESC, reason_code",
        )
        .fetch_all(&mut *transaction)
        .await?;

        let tokens = token_rows
            .into_iter()
            .map(|value| {
                let token: DiscoveryToken = serde_json::from_value(value)?;
                validate_token_shape(&token)?;
                if mode == DiscoveryMode::ApprovedOnly && token.stage != DiscoveryStage::Approved {
                    return Err(PersistenceError::InvalidStoredValue {
                        field: "discovery_tokens.stage",
                        value: discovery_stage_name(token.stage).to_owned(),
                    });
                }
                Ok(token)
            })
            .collect::<Result<Vec<_>, PersistenceError>>()?;
        let rejection_reasons = rejection_rows
            .into_iter()
            .map(|row| {
                Ok(RejectionSummary {
                    reason_code: row.reason_code,
                    count: to_u64(row.count, "discovery_rejection_summaries.count")?,
                    last_seen_unix_ms: row.last_seen_unix_ms,
                })
            })
            .collect::<Result<Vec<_>, PersistenceError>>()?;

        let tokens_total = to_u64(tokens_total, "discovery_tokens.visible_count")?;
        let tokens_truncated =
            tokens_total > u64::try_from(tokens.len()).expect("vector length always fits in u64");
        transaction.commit().await?;
        Ok(DiscoverySnapshot {
            sequence: to_u64(state.sequence, "discovery_projection_state.sequence")?,
            mode,
            tokens,
            tokens_total,
            tokens_truncated,
            counters: DiscoveryCounters {
                observed: to_u64(state.observed, "discovery_projection_state.observed")?,
                pending: to_u64(state.pending, "discovery_projection_state.pending")?,
                approved: to_u64(state.approved, "discovery_projection_state.approved")?,
                rejected: to_u64(state.rejected, "discovery_projection_state.rejected")?,
                flow_per_minute: state
                    .flow_per_minute
                    .map(|value| to_u64(value, "discovery_projection_state.flow_per_minute"))
                    .transpose()?,
            },
            rejection_reasons,
        })
    }

    /// Loads one retained candidate without materializing the full projection.
    /// Pump trade normalization uses this to recover the exact bonding-curve
    /// market identity established by an earlier create observation.
    pub async fn load_discovery_token(
        &self,
        mint: &str,
    ) -> Result<Option<DiscoveryToken>, PersistenceError> {
        require_non_empty(mint, "discovery mint")?;
        let token =
            sqlx::query_scalar::<_, Value>("SELECT token FROM discovery_tokens WHERE mint = $1")
                .bind(mint)
                .fetch_optional(&self.pool)
                .await?
                .map(|value| {
                    let token: DiscoveryToken = serde_json::from_value(value)?;
                    validate_token_shape(&token)?;
                    Ok::<DiscoveryToken, PersistenceError>(token)
                })
                .transpose()?;

        Ok(token)
    }

    pub async fn set_discovery_mode(&self, mode: DiscoveryMode) -> Result<bool, PersistenceError> {
        let mut transaction = self.pool.begin().await?;
        let sequence = sqlx::query_scalar::<_, i64>(
            "UPDATE discovery_projection_state \
             SET mode = $1, sequence = sequence + 1, updated_at = NOW() \
             WHERE singleton = TRUE AND mode <> $1 \
             RETURNING sequence",
        )
        .bind(discovery_mode_name(mode))
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some(sequence) = sequence {
            append_projection_event(
                &mut transaction,
                "DISCOVERY_MODE_CHANGED",
                json!({
                    "projection_sequence": sequence,
                    "mode": discovery_mode_name(mode),
                }),
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(sequence.is_some())
    }

    pub async fn set_discovery_flow_per_minute(
        &self,
        flow_per_minute: Option<u64>,
    ) -> Result<bool, PersistenceError> {
        let flow_per_minute = flow_per_minute
            .map(|value| to_i64(value, "flow_per_minute"))
            .transpose()?;
        let mut transaction = self.pool.begin().await?;
        let sequence = sqlx::query_scalar::<_, i64>(
            "UPDATE discovery_projection_state \
             SET flow_per_minute = $1, sequence = sequence + 1, \
                 updated_at = NOW() \
             WHERE singleton = TRUE AND flow_per_minute IS DISTINCT FROM $1 \
             RETURNING sequence",
        )
        .bind(flow_per_minute)
        .fetch_optional(&mut *transaction)
        .await?;
        if let Some(sequence) = sequence {
            append_projection_event(
                &mut transaction,
                "DISCOVERY_FLOW_CHANGED",
                json!({
                    "projection_sequence": sequence,
                    "flow_per_minute": flow_per_minute,
                }),
            )
            .await?;
        }
        transaction.commit().await?;
        Ok(sequence.is_some())
    }

    /// Repairs counters from durable tokens, work, and rejection summaries.
    ///
    /// `observed` is rebuilt as the number of currently retained structurally
    /// valid candidates; the normal write path keeps it as the same unique-mint
    /// count.
    pub async fn rebuild_discovery_projection(&self) -> Result<u64, PersistenceError> {
        let mut transaction = self.pool.begin().await?;
        let sequence = sqlx::query_scalar::<_, i64>(
            "WITH counts AS (\
                SELECT \
                    (SELECT COUNT(*) FROM discovery_tokens) AS observed, \
                    (SELECT COUNT(*) FROM discovery_tokens \
                        WHERE stage = 'APPROVED') AS approved, \
                    (SELECT COUNT(*) FROM observation_work \
                        WHERE work_kind = 'DISCOVERY' \
                          AND status IN ('PENDING', 'PROCESSING')) AS pending, \
                    (SELECT COALESCE(SUM(count), 0) \
                        FROM discovery_rejection_summaries) AS rejected\
             ) \
             UPDATE discovery_projection_state AS state \
             SET observed = counts.observed, \
                 approved = counts.approved, \
                 pending = counts.pending, \
                 rejected = counts.rejected, \
                 sequence = state.sequence + 1, \
                 updated_at = NOW() \
             FROM counts \
             WHERE state.singleton = TRUE \
             RETURNING state.sequence",
        )
        .fetch_one(&mut *transaction)
        .await?;
        append_projection_event(
            &mut transaction,
            "DISCOVERY_REBUILT",
            json!({ "projection_sequence": sequence }),
        )
        .await?;
        transaction.commit().await?;
        to_u64(sequence, "discovery_projection_state.sequence")
    }
}
