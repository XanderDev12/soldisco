use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use soldisco_api_contracts::{
    DiscoveryStage, DiscoveryToken, DiscoveryWindowSummary, PrefilterDefaults,
    QualificationDecision, QualificationDefaults, WindowCompleteness,
};
use soldisco_discovery_engine::{
    MarketWindowSnapshot, QualificationAssessment, QualificationPolicy, SnapshotCompleteness,
    WindowIncompleteReason, build_market_window_snapshot, evaluate_qualification,
};
use soldisco_domain::{
    AssessmentDecision, MarketIdentity, Network, NormalizedObservation, RuleResult, SourceProgram,
};
use sqlx::FromRow;

use crate::{
    Database, PersistenceError, StoredQualificationDefaults,
    observations::{enqueue_discovery_work, insert_chain_observation_unprepared},
    values::{
        assessment_decision_name, discovery_stage_name, network_name, parse_network,
        parse_source_program, parse_venue, source_program_name, to_i64, to_u64, venue_name,
    },
};

pub const MARKET_WINDOW_FEATURE_VERSION: &str = "market-window-features-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WindowTargetKind {
    PumpMint,
    PumpSwapPool,
}

impl WindowTargetKind {
    const fn database_name(self) -> &'static str {
        match self {
            Self::PumpMint => "PUMP_MINT",
            Self::PumpSwapPool => "PUMP_SWAP_POOL",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewDiscoveryWindow {
    pub target_kind: WindowTargetKind,
    pub target_address: String,
    pub opened_at_unix_ms: i64,
    pub closes_at_unix_ms: i64,
    pub collector_run_id: String,
    pub prefilter_revision: u64,
    pub prefilter_values: PrefilterDefaults,
    pub qualification: StoredQualificationDefaults,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenedDiscoveryWindow {
    pub window_id: i64,
    pub observation_inserted: bool,
    pub window_created: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryWindowRecord {
    pub window_id: i64,
    pub market: MarketIdentity,
    pub source_program: SourceProgram,
    pub opened_at_unix_ms: i64,
    pub closes_at_unix_ms: i64,
    pub collector_run_id: String,
    pub prefilter_revision: u64,
    pub prefilter_values: PrefilterDefaults,
    pub qualification_revision: u64,
    pub qualification_values: QualificationDefaults,
    pub observations: Vec<NormalizedObservation>,
}

/// Cheap durable precondition for rebuilding a qualification snapshot.
///
/// The finalizer uses this before loading the window's full observation
/// payload. Pending projection work is recoverable lag. A terminally failed
/// or missing opening projection is not recoverable by waiting and must make
/// the window incomplete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryWindowProjectionReadiness {
    Pending,
    Ready,
    OpeningProjectionUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizeDiscoveryWindow {
    pub window_id: i64,
    pub finalized_at_unix_ms: i64,
    pub completeness: WindowCompleteness,
    pub completeness_reason: Option<String>,
    pub feature_version: String,
    pub snapshot: Value,
    pub observation_count: u64,
    pub decision: AssessmentDecision,
    pub assessment: Value,
    pub rules: Vec<RuleResult>,
    pub summary: DiscoveryWindowSummary,
}

#[derive(Clone, Debug, FromRow)]
struct WindowIdentityRow {
    id: i64,
    network: String,
    source_program: String,
    venue: String,
    market_address: String,
    mint: String,
    quote_mint: String,
    target_kind: String,
    target_address: String,
    opened_at_unix_ms: i64,
    closes_at_unix_ms: i64,
    collector_run_id: String,
    prefilter_revision: i64,
    prefilter_values: Value,
    qualification_revision: i64,
    qualification_values: Value,
    status: String,
}

#[derive(Debug, FromRow)]
struct CurrentProjectionRow {
    token: Value,
    belongs_to_window: bool,
}

#[derive(Debug, FromRow)]
struct ProjectionReadinessRow {
    opening_work_status: Option<String>,
    projection_work_pending: bool,
    candidate_projection_present: bool,
}

#[derive(Debug, FromRow)]
struct StoredFinalizationRow {
    feature_version: String,
    observation_count: i64,
    snapshot: Value,
    decision: String,
    assessment: Value,
    summary: Value,
    completeness: String,
    completeness_reason: Option<String>,
}

/// Builds the one canonical finalization payload for a durable window.
///
/// The database independently rebuilds and compares this payload inside the
/// finalization transaction. Keeping the builder public lets the worker avoid
/// duplicating feature and rule logic without making submitted PASS evidence
/// authoritative.
pub fn build_discovery_window_finalization(
    window: &DiscoveryWindowRecord,
    finalized_at_unix_ms: i64,
    incomplete_reason: Option<&str>,
) -> Result<FinalizeDiscoveryWindow, PersistenceError> {
    let requested_completeness = match incomplete_reason {
        Some(reason_code) => SnapshotCompleteness::Incomplete {
            reason_code: reason_code.to_owned(),
        },
        None => SnapshotCompleteness::Complete,
    };
    let snapshot = build_market_window_snapshot(
        &window.market,
        window.opened_at_unix_ms,
        window.closes_at_unix_ms,
        requested_completeness,
        &window.observations,
    )?;
    let assessment = evaluate_qualification(&snapshot, &qualification_policy(window))?;
    let summary =
        canonical_discovery_summary(window, &snapshot, &assessment, finalized_at_unix_ms)?;
    let completeness = if snapshot.completeness.is_complete() {
        WindowCompleteness::Complete
    } else {
        WindowCompleteness::Incomplete
    };

    Ok(FinalizeDiscoveryWindow {
        window_id: window.window_id,
        finalized_at_unix_ms,
        completeness,
        completeness_reason: snapshot.completeness.reason_code().map(str::to_owned),
        feature_version: MARKET_WINDOW_FEATURE_VERSION.to_owned(),
        snapshot: serde_json::to_value(&snapshot)?,
        observation_count: snapshot.metrics.observations,
        decision: assessment.decision,
        assessment: serde_json::to_value(&assessment)?,
        rules: assessment.rules,
        summary,
    })
}

impl Database {
    /// Commits the authoritative creation observation and durable window
    /// identity together. The provisional in-memory token is confirmed only
    /// after this transaction succeeds.
    pub async fn open_discovery_window(
        &self,
        observation: &NormalizedObservation,
        window: &NewDiscoveryWindow,
    ) -> Result<OpenedDiscoveryWindow, PersistenceError> {
        validate_new_window(observation, window)?;
        let mut transaction = self.pool.begin().await?;
        lock_discovery_mint_projection(
            &mut transaction,
            network_name(observation.key.network),
            &observation.market.mint,
        )
        .await?;
        let inserted = insert_chain_observation_unprepared(&mut transaction, observation).await?;
        if inserted.is_new {
            enqueue_discovery_work(&mut transaction, observation, inserted.id).await?;
        }

        let window_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO discovery_windows (\
                opening_observation_id, opening_signature, \
                opening_instruction_index, opening_event_index, network, \
                source_program, venue, market_address, mint, quote_mint, \
                target_kind, target_address, opened_at_unix_ms, \
                closes_at_unix_ms, collector_run_id, prefilter_revision, \
                prefilter_values, qualification_revision, qualification_values\
             ) VALUES (\
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, \
                $13, $14, $15, $16, $17, $18, $19\
             ) ON CONFLICT (\
                network, source_program, opening_signature, \
                opening_instruction_index, opening_event_index, \
                target_kind, target_address\
             ) DO NOTHING \
             RETURNING id",
        )
        .bind(inserted.id)
        .bind(&observation.key.coordinate.signature)
        .bind(i32::from(observation.key.coordinate.instruction_index))
        .bind(i32::from(observation.key.coordinate.event_index))
        .bind(network_name(observation.key.network))
        .bind(source_program_name(observation.key.program))
        .bind(venue_name(observation.market.venue))
        .bind(&observation.market.market_address)
        .bind(&observation.market.mint)
        .bind(observation.market.quote_mint.as_deref().unwrap_or(""))
        .bind(window.target_kind.database_name())
        .bind(&window.target_address)
        .bind(window.opened_at_unix_ms)
        .bind(window.closes_at_unix_ms)
        .bind(&window.collector_run_id)
        .bind(to_i64(window.prefilter_revision, "prefilter_revision")?)
        .bind(serde_json::to_value(window.prefilter_values)?)
        .bind(to_i64(
            window.qualification.revision,
            "qualification_revision",
        )?)
        .bind(serde_json::to_value(window.qualification.values)?)
        .fetch_optional(&mut *transaction)
        .await?;
        let window_created = window_id.is_some();
        let window_id = match window_id {
            Some(id) => id,
            None => {
                let existing = load_window_identity_by_opening(
                    &mut transaction,
                    observation,
                    window.target_kind,
                    &window.target_address,
                )
                .await?;
                validate_existing_window(&existing, observation, window)?;
                let window_id = existing.id;
                transaction.rollback().await?;
                return Ok(OpenedDiscoveryWindow {
                    window_id,
                    observation_inserted: false,
                    window_created: false,
                    replayed: true,
                });
            }
        };

        insert_membership(
            &mut transaction,
            window_id,
            inserted.id,
            observation.received_time_unix_ms,
        )
        .await?;
        if window_created {
            let sequence = sqlx::query_scalar::<_, i64>(
                "UPDATE discovery_projection_state \
                 SET qualification_pending = qualification_pending + 1, \
                     sequence = sequence + 1, updated_at = NOW() \
                 WHERE singleton = TRUE \
                 RETURNING sequence",
            )
            .fetch_one(&mut *transaction)
            .await?;
            sqlx::query(
                "INSERT INTO projection_events (projection_name, event_kind, payload) \
                 VALUES ('DISCOVERY', 'QUALIFICATION_WINDOW_OPENED', $1)",
            )
            .bind(json!({
                "projection_sequence": sequence,
                "window_id": window_id,
                "mint": observation.market.mint,
                "qualification_revision": window.qualification.revision,
            }))
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;

        Ok(OpenedDiscoveryWindow {
            window_id,
            observation_inserted: inserted.is_new,
            window_created,
            replayed: false,
        })
    }

    /// Persists one exact fact and its membership atomically. Receipt time,
    /// not processor time, decides half-open window membership.
    pub async fn insert_window_observation(
        &self,
        window_id: i64,
        observation: &NormalizedObservation,
    ) -> Result<bool, PersistenceError> {
        let mut transaction = self.pool.begin().await?;
        let window = load_window_identity_by_id(&mut transaction, window_id).await?;
        let inserted = insert_chain_observation_unprepared(&mut transaction, observation).await?;
        if !inserted.is_new {
            let already_admitted = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (\
                    SELECT 1 \
                    FROM discovery_window_observations \
                    WHERE window_id = $1 AND observation_id = $2\
                 )",
            )
            .bind(window_id)
            .bind(inserted.id)
            .fetch_one(&mut *transaction)
            .await?;
            if already_admitted {
                transaction.commit().await?;
                return Ok(false);
            }
        }

        let admitted_observation = if inserted.is_new {
            observation.clone()
        } else {
            let stored = sqlx::query_scalar::<_, Value>(
                "SELECT normalized_observation FROM chain_observations WHERE id = $1",
            )
            .bind(inserted.id)
            .fetch_one(&mut *transaction)
            .await?;
            serde_json::from_value(stored)?
        };
        validate_window_membership(&window, &admitted_observation)?;
        if inserted.is_new {
            enqueue_discovery_work(&mut transaction, observation, inserted.id).await?;
        }
        insert_membership(
            &mut transaction,
            window_id,
            inserted.id,
            admitted_observation.received_time_unix_ms,
        )
        .await?;
        transaction.commit().await?;
        Ok(inserted.is_new)
    }

    pub async fn load_discovery_window(
        &self,
        window_id: i64,
    ) -> Result<DiscoveryWindowRecord, PersistenceError> {
        let row = sqlx::query_as::<_, WindowIdentityRow>(
            "SELECT id, network, source_program, venue, market_address, mint, \
                    quote_mint, target_kind, target_address, \
                    opened_at_unix_ms, closes_at_unix_ms, collector_run_id, \
                    prefilter_revision, prefilter_values, \
                    qualification_revision, qualification_values, status \
             FROM discovery_windows \
             WHERE id = $1",
        )
        .bind(window_id)
        .fetch_one(&self.pool)
        .await?;
        let observation_values = sqlx::query_scalar::<_, Value>(
            "SELECT observation.normalized_observation \
             FROM discovery_window_observations AS member \
             JOIN chain_observations AS observation \
               ON observation.id = member.observation_id \
             WHERE member.window_id = $1 \
             ORDER BY observation.received_time_unix_ms, observation.slot, \
                      observation.signature, observation.instruction_index, \
                      observation.event_index",
        )
        .bind(window_id)
        .fetch_all(&self.pool)
        .await?;
        let observations = observation_values
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<Vec<_>, _>>()?;
        row.into_record(observations)
    }

    /// Checks whether browser projection work is ready without loading or
    /// deserializing the window's full observation evidence.
    pub async fn load_discovery_window_projection_readiness(
        &self,
        window_id: i64,
    ) -> Result<DiscoveryWindowProjectionReadiness, PersistenceError> {
        let row = sqlx::query_as::<_, ProjectionReadinessRow>(
            "SELECT opening_work.status AS opening_work_status, \
                    EXISTS (\
                        SELECT 1 \
                        FROM discovery_window_observations AS member \
                        JOIN observation_work AS work \
                          ON work.observation_id = member.observation_id \
                        WHERE member.window_id = discovery_window.id \
                          AND work.work_kind = 'DISCOVERY' \
                          AND work.status IN ('PENDING', 'PROCESSING')\
                    ) AS projection_work_pending, \
                    EXISTS (\
                        SELECT 1 \
                        FROM discovery_tokens AS candidate \
                        WHERE candidate.mint = discovery_window.mint\
                    ) AS candidate_projection_present \
             FROM discovery_windows AS discovery_window \
             LEFT JOIN observation_work AS opening_work \
               ON opening_work.observation_id = \
                    discovery_window.opening_observation_id \
              AND opening_work.work_kind = 'DISCOVERY' \
             WHERE discovery_window.id = $1",
        )
        .bind(window_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PersistenceError::DiscoveryWindowNotActive { window_id })?;

        Ok(projection_readiness(row))
    }

    pub async fn load_active_discovery_window_ids(
        &self,
        collector_run_id: Option<&str>,
    ) -> Result<Vec<i64>, PersistenceError> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT id \
             FROM discovery_windows \
             WHERE status = 'ACTIVE' \
               AND ($1::TEXT IS NULL OR collector_run_id = $1) \
             ORDER BY closes_at_unix_ms, id",
        )
        .bind(collector_run_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Returns active windows owned by a prior collector run. The current
    /// process retries these after the durable discovery worker has had a
    /// chance to rebuild any browser projection that was still queued when the
    /// prior process stopped.
    pub async fn load_interrupted_discovery_window_ids(
        &self,
        current_collector_run_id: &str,
    ) -> Result<Vec<i64>, PersistenceError> {
        if current_collector_run_id.trim().is_empty() {
            return Err(PersistenceError::EmptyField {
                field: "current_collector_run_id",
            });
        }
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT id \
             FROM discovery_windows \
             WHERE status = 'ACTIVE' \
               AND collector_run_id <> $1 \
             ORDER BY closes_at_unix_ms, id",
        )
        .bind(current_collector_run_id)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Atomically freezes window evidence, saves every qualification rule,
    /// advances the exact current candidate, updates counters, and emits one
    /// projection invalidation. A crash can therefore never publish a pass
    /// without its immutable evidence.
    pub async fn finalize_discovery_window(
        &self,
        finalized: &FinalizeDiscoveryWindow,
    ) -> Result<bool, PersistenceError> {
        validate_finalization(finalized)?;
        let mut transaction = self.pool.begin().await?;
        let (locked_network, locked_mint) =
            load_window_mint_lock_identity(&mut transaction, finalized.window_id).await?;
        lock_discovery_mint_projection(&mut transaction, &locked_network, &locked_mint).await?;
        let window = load_window_identity_by_id(&mut transaction, finalized.window_id).await?;
        if window.network != locked_network || window.mint != locked_mint {
            transaction.rollback().await?;
            return Err(PersistenceError::DiscoveryWindowIdentityConflict {
                window_id: finalized.window_id,
            });
        }
        validate_finalization_for_window(&window, finalized)?;
        if window.status == "FINALIZED" {
            let existing = sqlx::query_as::<_, StoredFinalizationRow>(
                "SELECT snapshot.feature_version, snapshot.observation_count, \
                        snapshot.snapshot, assessment.decision, \
                        assessment.assessment, assessment.summary, \
                        dw.completeness, dw.completeness_reason \
                 FROM discovery_windows AS dw \
                 JOIN discovery_window_snapshots AS snapshot \
                   ON snapshot.window_id = dw.id \
                 JOIN qualification_assessments AS assessment \
                   ON assessment.window_id = dw.id \
                 WHERE dw.id = $1",
            )
            .bind(finalized.window_id)
            .fetch_optional(&mut *transaction)
            .await?;
            transaction.rollback().await?;
            return if existing
                .as_ref()
                .is_some_and(|existing| finalization_matches(existing, finalized))
            {
                Ok(false)
            } else {
                Err(PersistenceError::DiscoveryWindowFinalizationConflict {
                    window_id: finalized.window_id,
                })
            };
        }

        let readiness =
            load_projection_readiness_in_transaction(&mut transaction, finalized.window_id).await?;
        if readiness == DiscoveryWindowProjectionReadiness::Pending {
            transaction.rollback().await?;
            return Err(PersistenceError::DiscoveryWindowProjectionMissing {
                window_id: finalized.window_id,
            });
        }
        if readiness == DiscoveryWindowProjectionReadiness::OpeningProjectionUnavailable
            && finalized.completeness_reason.as_deref()
                != Some(WindowIncompleteReason::ProcessingFailed.as_str())
        {
            transaction.rollback().await?;
            return Err(
                PersistenceError::DiscoveryWindowFinalizationEvidenceMismatch {
                    window_id: finalized.window_id,
                },
            );
        }

        let durable_observation_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) \
             FROM discovery_window_observations \
             WHERE window_id = $1",
        )
        .bind(finalized.window_id)
        .fetch_one(&mut *transaction)
        .await?;
        if durable_observation_count
            != to_i64(finalized.observation_count, "window observation_count")?
        {
            transaction.rollback().await?;
            return Err(PersistenceError::DiscoveryWindowEvidenceChanged {
                window_id: finalized.window_id,
            });
        }
        let observations = load_window_observations(&mut transaction, finalized.window_id).await?;
        let canonical_window = window.clone().into_record(observations)?;
        let canonical = build_discovery_window_finalization(
            &canonical_window,
            finalized.finalized_at_unix_ms,
            finalized.completeness_reason.as_deref(),
        )?;
        if canonical != *finalized {
            transaction.rollback().await?;
            return Err(
                PersistenceError::DiscoveryWindowFinalizationEvidenceMismatch {
                    window_id: finalized.window_id,
                },
            );
        }

        let current_projection = sqlx::query_as::<_, CurrentProjectionRow>(
            "SELECT candidate.token, \
                    EXISTS (\
                        SELECT 1 \
                        FROM observation_work AS work \
                        JOIN discovery_window_observations AS member \
                          ON member.observation_id = work.observation_id \
                        WHERE work.id = candidate.source_work_id \
                          AND member.window_id = $2\
                    ) AS belongs_to_window \
             FROM discovery_tokens AS candidate \
             WHERE candidate.mint = $1 \
             FOR UPDATE",
        )
        .bind(&window.mint)
        .bind(finalized.window_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let newer_window_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (\
                SELECT 1 \
                FROM discovery_windows AS newer \
                WHERE newer.mint = $1 \
                  AND newer.id <> $2 \
                  AND (\
                    newer.opened_at_unix_ms > $3 \
                    OR (newer.opened_at_unix_ms = $3 AND newer.id > $2)\
                  )\
             )",
        )
        .bind(&window.mint)
        .bind(finalized.window_id)
        .bind(window.opened_at_unix_ms)
        .fetch_one(&mut *transaction)
        .await?;
        let projection_is_current = current_projection
            .as_ref()
            .is_some_and(|projection| projection.belongs_to_window)
            && !newer_window_exists;

        let snapshot_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO discovery_window_snapshots (\
                window_id, feature_version, observation_count, snapshot\
             ) VALUES ($1, $2, $3, $4) \
             RETURNING id",
        )
        .bind(finalized.window_id)
        .bind(&finalized.feature_version)
        .bind(to_i64(
            finalized.observation_count,
            "window observation_count",
        )?)
        .bind(&finalized.snapshot)
        .fetch_one(&mut *transaction)
        .await?;
        let assessment_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO qualification_assessments (\
                window_id, window_snapshot_id, ruleset_revision, decision, \
                assessment, summary\
             ) VALUES ($1, $2, $3, $4, $5, $6) \
             RETURNING id",
        )
        .bind(finalized.window_id)
        .bind(snapshot_id)
        .bind(window.qualification_revision)
        .bind(assessment_decision_name(finalized.decision))
        .bind(&finalized.assessment)
        .bind(serde_json::to_value(&finalized.summary)?)
        .fetch_one(&mut *transaction)
        .await?;
        for rule in &finalized.rules {
            sqlx::query(
                "INSERT INTO qualification_rule_results (\
                    assessment_id, rule_id, rule_version, decision, reason_code, \
                    evidence_reference, observed_slot\
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(assessment_id)
            .bind(&rule.rule_id)
            .bind(&rule.rule_version)
            .bind(assessment_decision_name(rule.decision))
            .bind(&rule.reason_code)
            .bind(&rule.evidence_reference)
            .bind(
                rule.observed_slot
                    .map(|slot| to_i64(slot, "qualification observed_slot"))
                    .transpose()?,
            )
            .execute(&mut *transaction)
            .await?;
        }

        let (qualified_delta, approval_removed) = if projection_is_current {
            let mut token: DiscoveryToken = serde_json::from_value(
                current_projection
                    .expect("a current projection row was checked above")
                    .token,
            )?;
            validate_projection_matches_window(&token, &window)?;
            let was_qualified = matches!(
                token.stage,
                DiscoveryStage::Qualified | DiscoveryStage::Approved
            );
            let was_approved = token.stage == DiscoveryStage::Approved;
            let now_qualified = finalized.decision == AssessmentDecision::Pass;
            token.stage = if now_qualified {
                if was_approved {
                    DiscoveryStage::Approved
                } else {
                    DiscoveryStage::Qualified
                }
            } else {
                DiscoveryStage::Observed
            };
            token.qualification = Some(finalized.summary.clone());
            token.activity.trades = finalized.summary.trades;
            token.activity.buys = finalized.summary.buys;
            token.activity.sells = finalized.summary.sells;
            token.activity.unique_traders = finalized.summary.unique_traders;
            token.activity.base_volume_units = checked_decimal_sum(
                &finalized.summary.buy_base_volume_units,
                &finalized.summary.sell_base_volume_units,
                "qualification base volume",
            )?;
            token.activity.quote_volume_units = checked_decimal_sum(
                &finalized.summary.buy_quote_volume_units,
                &finalized.summary.sell_quote_volume_units,
                "qualification quote volume",
            )?;

            sqlx::query(
                "UPDATE discovery_tokens \
                 SET stage = $2, decision_source = 'WINDOW_QUALIFICATION', \
                     decision_version = $3, token = $4, updated_at = NOW() \
                 WHERE mint = $1",
            )
            .bind(&token.mint)
            .bind(discovery_stage_name(token.stage))
            .bind(window.qualification_revision.to_string())
            .bind(serde_json::to_value(&token)?)
            .execute(&mut *transaction)
            .await?;
            (
                i64::from(now_qualified) - i64::from(was_qualified),
                i64::from(was_approved && !now_qualified),
            )
        } else {
            (0, 0)
        };

        sqlx::query(
            "UPDATE discovery_windows \
             SET status = 'FINALIZED', completeness = $2, \
                 completeness_reason = $3, finalized_at_unix_ms = $4, \
                 updated_at = NOW() \
             WHERE id = $1",
        )
        .bind(finalized.window_id)
        .bind(match finalized.completeness {
            WindowCompleteness::Complete => "COMPLETE",
            WindowCompleteness::Incomplete => "INCOMPLETE",
        })
        .bind(&finalized.completeness_reason)
        .bind(finalized.finalized_at_unix_ms)
        .execute(&mut *transaction)
        .await?;

        let sequence = sqlx::query_scalar::<_, i64>(
            "UPDATE discovery_projection_state \
             SET qualification_pending = GREATEST(qualification_pending - 1, 0), \
                 qualified = GREATEST(qualified + $1, 0), \
                 qualification_rejected = qualification_rejected + $2, \
                 qualification_unknown = qualification_unknown + $3, \
                 approved = GREATEST(approved - $4, 0), \
                 sequence = sequence + 1, updated_at = NOW() \
             WHERE singleton = TRUE \
             RETURNING sequence",
        )
        .bind(qualified_delta)
        .bind(i64::from(finalized.decision == AssessmentDecision::Reject))
        .bind(i64::from(finalized.decision == AssessmentDecision::Unknown))
        .bind(approval_removed)
        .fetch_one(&mut *transaction)
        .await?;

        let summary_reasons = finalized
            .rules
            .iter()
            .filter(|rule| rule.decision == finalized.decision)
            .map(|rule| rule.reason_code.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        for reason in summary_reasons {
            match finalized.decision {
                AssessmentDecision::Reject => {
                    upsert_decision_summary(
                        &mut transaction,
                        "qualification_rejection_summaries",
                        reason,
                        finalized.finalized_at_unix_ms,
                    )
                    .await?;
                }
                AssessmentDecision::Unknown => {
                    upsert_decision_summary(
                        &mut transaction,
                        "qualification_unknown_summaries",
                        reason,
                        finalized.finalized_at_unix_ms,
                    )
                    .await?;
                }
                AssessmentDecision::Pass => {}
            }
        }

        sqlx::query(
            "INSERT INTO projection_events (projection_name, event_kind, payload) \
             VALUES ('DISCOVERY', 'QUALIFICATION_WINDOW_FINALIZED', $1)",
        )
        .bind(json!({
            "projection_sequence": sequence,
            "window_id": finalized.window_id,
            "mint": window.mint,
            "decision": assessment_decision_name(finalized.decision),
            "ruleset_revision": window.qualification_revision,
            "projection_applied": projection_is_current,
        }))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(true)
    }
}

async fn load_projection_readiness_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    window_id: i64,
) -> Result<DiscoveryWindowProjectionReadiness, PersistenceError> {
    let row = sqlx::query_as::<_, ProjectionReadinessRow>(
        "SELECT opening_work.status AS opening_work_status, \
                EXISTS (\
                    SELECT 1 \
                    FROM discovery_window_observations AS member \
                    JOIN observation_work AS work \
                      ON work.observation_id = member.observation_id \
                    WHERE member.window_id = discovery_window.id \
                      AND work.work_kind = 'DISCOVERY' \
                      AND work.status IN ('PENDING', 'PROCESSING')\
                ) AS projection_work_pending, \
                EXISTS (\
                    SELECT 1 \
                    FROM discovery_tokens AS candidate \
                    WHERE candidate.mint = discovery_window.mint\
                ) AS candidate_projection_present \
         FROM discovery_windows AS discovery_window \
         LEFT JOIN observation_work AS opening_work \
           ON opening_work.observation_id = discovery_window.opening_observation_id \
          AND opening_work.work_kind = 'DISCOVERY' \
         WHERE discovery_window.id = $1",
    )
    .bind(window_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(PersistenceError::DiscoveryWindowNotActive { window_id })?;
    Ok(projection_readiness(row))
}

fn projection_readiness(row: ProjectionReadinessRow) -> DiscoveryWindowProjectionReadiness {
    if row.projection_work_pending {
        DiscoveryWindowProjectionReadiness::Pending
    } else if row.opening_work_status.as_deref() == Some("COMPLETE")
        && row.candidate_projection_present
    {
        DiscoveryWindowProjectionReadiness::Ready
    } else {
        DiscoveryWindowProjectionReadiness::OpeningProjectionUnavailable
    }
}

async fn upsert_decision_summary(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: &'static str,
    reason_code: &str,
    seen_at_unix_ms: i64,
) -> Result<(), PersistenceError> {
    let query = match table {
        "qualification_rejection_summaries" => {
            "INSERT INTO qualification_rejection_summaries (\
                reason_code, count, last_seen_unix_ms\
             ) VALUES ($1, 1, $2) \
             ON CONFLICT (reason_code) DO UPDATE \
             SET count = qualification_rejection_summaries.count + 1, \
                 last_seen_unix_ms = GREATEST(\
                    qualification_rejection_summaries.last_seen_unix_ms, \
                    EXCLUDED.last_seen_unix_ms\
                 ), updated_at = NOW()"
        }
        "qualification_unknown_summaries" => {
            "INSERT INTO qualification_unknown_summaries (\
                reason_code, count, last_seen_unix_ms\
             ) VALUES ($1, 1, $2) \
             ON CONFLICT (reason_code) DO UPDATE \
             SET count = qualification_unknown_summaries.count + 1, \
                 last_seen_unix_ms = GREATEST(\
                    qualification_unknown_summaries.last_seen_unix_ms, \
                    EXCLUDED.last_seen_unix_ms\
                 ), updated_at = NOW()"
        }
        _ => unreachable!("decision summary table is a static internal choice"),
    };
    sqlx::query(query)
        .bind(reason_code)
        .bind(seen_at_unix_ms)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

fn qualification_policy(window: &DiscoveryWindowRecord) -> QualificationPolicy {
    let QualificationDefaults {
        minimum_trades,
        minimum_unique_traders,
        minimum_buys,
        minimum_sells,
        minimum_native_quote_volume_units,
        minimum_stable_quote_volume_units,
        maximum_single_wallet_quote_share_bps,
    } = window.qualification_values;
    QualificationPolicy {
        ruleset_version: format!("qualification-revision-{}", window.qualification_revision),
        minimum_trades: u64::from(minimum_trades),
        minimum_unique_traders: u64::from(minimum_unique_traders),
        minimum_buys: u64::from(minimum_buys),
        minimum_sells: u64::from(minimum_sells),
        minimum_native_quote_volume_units: u128::from(minimum_native_quote_volume_units),
        minimum_stable_quote_volume_units: u128::from(minimum_stable_quote_volume_units),
        maximum_single_wallet_quote_volume_bps: maximum_single_wallet_quote_share_bps,
    }
}

fn canonical_discovery_summary(
    window: &DiscoveryWindowRecord,
    snapshot: &MarketWindowSnapshot,
    assessment: &QualificationAssessment,
    evaluated_unix_ms: i64,
) -> Result<DiscoveryWindowSummary, PersistenceError> {
    let decision = match assessment.decision {
        AssessmentDecision::Pass => QualificationDecision::Pass,
        AssessmentDecision::Reject => QualificationDecision::Reject,
        AssessmentDecision::Unknown => QualificationDecision::Unknown,
    };
    let completeness = if snapshot.completeness.is_complete() {
        WindowCompleteness::Complete
    } else {
        WindowCompleteness::Incomplete
    };
    let reason_codes = assessment
        .rules
        .iter()
        .filter(|rule| rule.decision == assessment.decision)
        .map(|rule| rule.reason_code.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let first_reserves = snapshot.reserves.as_ref().map(|reserves| &reserves.first);
    let latest_reserves = snapshot.reserves.as_ref().map(|reserves| &reserves.latest);

    Ok(DiscoveryWindowSummary {
        window_revision: u64::try_from(window.window_id)
            .map_err(|_| PersistenceError::ValueOutOfRange { field: "window id" })?,
        ruleset_revision: window.qualification_revision,
        opened_unix_ms: window.opened_at_unix_ms,
        closed_unix_ms: window.closes_at_unix_ms,
        evaluated_unix_ms,
        decision,
        completeness,
        reason_codes,
        trades: snapshot.metrics.trades,
        buys: snapshot.metrics.buys,
        sells: snapshot.metrics.sells,
        unique_traders: snapshot.metrics.unique_traders,
        unique_buyers: snapshot.metrics.unique_buyers,
        unique_sellers: snapshot.metrics.unique_sellers,
        buy_base_volume_units: snapshot.metrics.buy_base_units.to_string(),
        sell_base_volume_units: snapshot.metrics.sell_base_units.to_string(),
        buy_quote_volume_units: snapshot.metrics.buy_quote_units.to_string(),
        sell_quote_volume_units: snapshot.metrics.sell_quote_units.to_string(),
        maximum_single_wallet_quote_share_bps: snapshot
            .metrics
            .largest_wallet_quote_volume
            .as_ref()
            .map(|largest| largest.share_bps),
        price_change_bps: snapshot
            .prices
            .as_ref()
            .and_then(|prices| prices.change_bps)
            .and_then(|change_bps| i64::try_from(change_bps).ok()),
        first_base_reserve_units: first_reserves
            .and_then(|reserves| reserves.base_units)
            .map(|units| units.to_string()),
        first_quote_reserve_units: first_reserves
            .and_then(|reserves| reserves.quote_units)
            .map(|units| units.to_string()),
        latest_base_reserve_units: latest_reserves
            .and_then(|reserves| reserves.base_units)
            .map(|units| units.to_string()),
        latest_quote_reserve_units: latest_reserves
            .and_then(|reserves| reserves.quote_units)
            .map(|units| units.to_string()),
    })
}

fn checked_decimal_sum(
    left: &str,
    right: &str,
    field: &'static str,
) -> Result<String, PersistenceError> {
    let left = left
        .parse::<u128>()
        .map_err(|_| PersistenceError::InvalidUnsignedDecimal { field })?;
    let right = right
        .parse::<u128>()
        .map_err(|_| PersistenceError::InvalidUnsignedDecimal { field })?;
    left.checked_add(right)
        .map(|value| value.to_string())
        .ok_or(PersistenceError::ValueOutOfRange { field })
}

fn validate_projection_matches_window(
    token: &DiscoveryToken,
    window: &WindowIdentityRow,
) -> Result<(), PersistenceError> {
    if token.mint != window.mint
        || source_program_name(token.source_program) != window.source_program
        || venue_name(token.primary_venue) != window.venue
        || token.market_address != window.market_address
        || token.quote_mint.as_deref().unwrap_or("") != window.quote_mint
    {
        return Err(PersistenceError::DiscoveryWindowIdentityConflict {
            window_id: window.id,
        });
    }
    Ok(())
}

fn finalization_matches(
    stored: &StoredFinalizationRow,
    finalized: &FinalizeDiscoveryWindow,
) -> bool {
    let Ok(mut stored_summary) =
        serde_json::from_value::<DiscoveryWindowSummary>(stored.summary.clone())
    else {
        return false;
    };
    let mut submitted_summary = finalized.summary.clone();
    stored_summary.evaluated_unix_ms = 0;
    submitted_summary.evaluated_unix_ms = 0;
    stored.feature_version == finalized.feature_version
        && stored.observation_count
            == i64::try_from(finalized.observation_count).unwrap_or(i64::MIN)
        && stored.snapshot == finalized.snapshot
        && stored.decision == assessment_decision_name(finalized.decision)
        && stored.assessment == finalized.assessment
        && stored_summary == submitted_summary
        && stored.completeness
            == match finalized.completeness {
                WindowCompleteness::Complete => "COMPLETE",
                WindowCompleteness::Incomplete => "INCOMPLETE",
            }
        && stored.completeness_reason == finalized.completeness_reason
}

fn validate_finalization_for_window(
    window: &WindowIdentityRow,
    finalized: &FinalizeDiscoveryWindow,
) -> Result<(), PersistenceError> {
    let expected_window_revision = u64::try_from(window.id)
        .map_err(|_| PersistenceError::ValueOutOfRange { field: "window id" })?;
    let expected_ruleset_revision =
        to_u64(window.qualification_revision, "qualification_revision")?;
    if finalized.summary.window_revision != expected_window_revision
        || finalized.summary.ruleset_revision != expected_ruleset_revision
        || finalized.summary.opened_unix_ms != window.opened_at_unix_ms
        || finalized.summary.closed_unix_ms != window.closes_at_unix_ms
        || finalized.summary.evaluated_unix_ms != finalized.finalized_at_unix_ms
        || finalized.summary.completeness != finalized.completeness
    {
        return Err(PersistenceError::DiscoveryWindowIdentityConflict {
            window_id: window.id,
        });
    }
    let expected_reasons = finalized
        .rules
        .iter()
        .filter(|rule| rule.decision == finalized.decision)
        .map(|rule| rule.reason_code.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let submitted_reasons = finalized
        .summary
        .reason_codes
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if expected_reasons != submitted_reasons
        || submitted_reasons.len() != finalized.summary.reason_codes.len()
        || finalized.summary.buys.checked_add(finalized.summary.sells)
            != Some(finalized.summary.trades)
        || finalized.summary.unique_traders > finalized.summary.trades
        || finalized.summary.unique_buyers > finalized.summary.buys
        || finalized.summary.unique_sellers > finalized.summary.sells
        || finalized
            .summary
            .maximum_single_wallet_quote_share_bps
            .is_some_and(|share| share > 10_000)
    {
        return Err(PersistenceError::ScreeningDecisionMismatch);
    }
    checked_decimal_sum(
        &finalized.summary.buy_base_volume_units,
        &finalized.summary.sell_base_volume_units,
        "qualification base volume",
    )?;
    checked_decimal_sum(
        &finalized.summary.buy_quote_volume_units,
        &finalized.summary.sell_quote_volume_units,
        "qualification quote volume",
    )?;
    Ok(())
}

fn validate_finalization(finalized: &FinalizeDiscoveryWindow) -> Result<(), PersistenceError> {
    if finalized.feature_version.trim().is_empty() {
        return Err(PersistenceError::EmptyField {
            field: "feature_version",
        });
    }
    if finalized.completeness == WindowCompleteness::Complete
        && finalized.finalized_at_unix_ms < finalized.summary.closed_unix_ms
    {
        return Err(PersistenceError::InvalidStoredValue {
            field: "window finalized_at_unix_ms",
            value: finalized.finalized_at_unix_ms.to_string(),
        });
    }
    let mut rule_ids = std::collections::BTreeSet::new();
    for rule in &finalized.rules {
        if rule.rule_id.trim().is_empty()
            || rule.rule_version.trim().is_empty()
            || rule.reason_code.trim().is_empty()
        {
            return Err(PersistenceError::EmptyField {
                field: "qualification rule",
            });
        }
        if !rule_ids.insert(&rule.rule_id) {
            return Err(PersistenceError::InvalidStoredValue {
                field: "qualification rule_id",
                value: rule.rule_id.clone(),
            });
        }
    }
    let expected_decision = match finalized.summary.decision {
        QualificationDecision::Pass => AssessmentDecision::Pass,
        QualificationDecision::Reject => AssessmentDecision::Reject,
        QualificationDecision::Unknown => AssessmentDecision::Unknown,
    };
    if expected_decision != finalized.decision {
        return Err(PersistenceError::ScreeningDecisionMismatch);
    }
    match finalized.completeness {
        WindowCompleteness::Complete if finalized.completeness_reason.is_some() => {
            return Err(PersistenceError::InvalidStoredValue {
                field: "window completeness_reason",
                value: "present for COMPLETE".to_owned(),
            });
        }
        WindowCompleteness::Incomplete
            if finalized
                .completeness_reason
                .as_deref()
                .is_none_or(|reason| reason.trim().is_empty()) =>
        {
            return Err(PersistenceError::EmptyField {
                field: "window completeness_reason",
            });
        }
        WindowCompleteness::Complete | WindowCompleteness::Incomplete => {}
    }
    let aggregate = if finalized
        .rules
        .iter()
        .any(|rule| rule.decision == AssessmentDecision::Reject)
    {
        AssessmentDecision::Reject
    } else if finalized.rules.is_empty()
        || finalized
            .rules
            .iter()
            .any(|rule| rule.decision == AssessmentDecision::Unknown)
    {
        AssessmentDecision::Unknown
    } else {
        AssessmentDecision::Pass
    };
    if aggregate != finalized.decision {
        return Err(PersistenceError::ScreeningDecisionMismatch);
    }
    Ok(())
}

async fn insert_membership(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    window_id: i64,
    observation_id: i64,
    admitted_at_unix_ms: i64,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "INSERT INTO discovery_window_observations (\
            window_id, observation_id, admitted_at_unix_ms\
         ) VALUES ($1, $2, $3) \
         ON CONFLICT (window_id, observation_id) DO UPDATE \
         SET admitted_at_unix_ms = LEAST(\
             discovery_window_observations.admitted_at_unix_ms, \
             EXCLUDED.admitted_at_unix_ms\
         )",
    )
    .bind(window_id)
    .bind(observation_id)
    .bind(admitted_at_unix_ms)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn load_window_observations(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    window_id: i64,
) -> Result<Vec<NormalizedObservation>, PersistenceError> {
    sqlx::query_scalar::<_, Value>(
        "SELECT observation.normalized_observation \
         FROM discovery_window_observations AS member \
         JOIN chain_observations AS observation \
           ON observation.id = member.observation_id \
         WHERE member.window_id = $1 \
         ORDER BY observation.received_time_unix_ms, observation.slot, \
                  observation.signature, observation.instruction_index, \
                  observation.event_index",
    )
    .bind(window_id)
    .fetch_all(&mut **transaction)
    .await?
    .into_iter()
    .map(serde_json::from_value)
    .collect::<Result<Vec<_>, _>>()
    .map_err(PersistenceError::from)
}

/// Serializes window identity and browser-projection decisions for one
/// `(network, mint)` until the surrounding transaction commits or rolls back.
///
/// Both window opening and finalization acquire this advisory lock before any
/// row-level identity or projection lock. That ordering makes a finalizer wait
/// for an in-flight newer window and then evaluate it from a fresh READ
/// COMMITTED statement snapshot. Each current caller acquires exactly one key;
/// a future operation that needs several must acquire them in sorted
/// `(network, mint)` order. A 64-bit hash collision only over-serializes
/// unrelated mints and cannot bypass the boundary.
async fn lock_discovery_mint_projection(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    network: &str,
    mint: &str,
) -> Result<(), PersistenceError> {
    sqlx::query(
        "SELECT pg_advisory_xact_lock(\
            hashtextextended($2, hashtextextended($1, 0::BIGINT))\
         )",
    )
    .bind(network)
    .bind(mint)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Reads only the immutable key needed to choose the per-mint advisory lock.
/// No database row lock may be taken before the advisory lock is acquired.
async fn load_window_mint_lock_identity(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    window_id: i64,
) -> Result<(String, String), PersistenceError> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT network, mint \
         FROM discovery_windows \
         WHERE id = $1",
    )
    .bind(window_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(PersistenceError::DiscoveryWindowNotActive { window_id })
}

async fn load_window_identity_by_id(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    window_id: i64,
) -> Result<WindowIdentityRow, PersistenceError> {
    sqlx::query_as::<_, WindowIdentityRow>(
        "SELECT id, network, source_program, venue, market_address, mint, \
                quote_mint, target_kind, target_address, opened_at_unix_ms, \
                closes_at_unix_ms, collector_run_id, prefilter_revision, \
                prefilter_values, qualification_revision, \
                qualification_values, status \
         FROM discovery_windows \
         WHERE id = $1 \
         FOR UPDATE",
    )
    .bind(window_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or(PersistenceError::DiscoveryWindowNotActive { window_id })
}

async fn load_window_identity_by_opening(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    observation: &NormalizedObservation,
    target_kind: WindowTargetKind,
    target_address: &str,
) -> Result<WindowIdentityRow, PersistenceError> {
    sqlx::query_as::<_, WindowIdentityRow>(
        "SELECT id, network, source_program, venue, market_address, mint, \
                quote_mint, target_kind, target_address, opened_at_unix_ms, \
                closes_at_unix_ms, collector_run_id, prefilter_revision, \
                prefilter_values, qualification_revision, \
                qualification_values, status \
         FROM discovery_windows \
         WHERE network = $1 \
           AND source_program = $2 \
           AND opening_signature = $3 \
           AND opening_instruction_index = $4 \
           AND opening_event_index = $5 \
           AND target_kind = $6 \
           AND target_address = $7 \
         FOR UPDATE",
    )
    .bind(network_name(observation.key.network))
    .bind(source_program_name(observation.key.program))
    .bind(&observation.key.coordinate.signature)
    .bind(i32::from(observation.key.coordinate.instruction_index))
    .bind(i32::from(observation.key.coordinate.event_index))
    .bind(target_kind.database_name())
    .bind(target_address)
    .fetch_one(&mut **transaction)
    .await
    .map_err(PersistenceError::from)
}

fn validate_new_window(
    observation: &NormalizedObservation,
    window: &NewDiscoveryWindow,
) -> Result<(), PersistenceError> {
    if window.closes_at_unix_ms <= window.opened_at_unix_ms {
        return Err(PersistenceError::InvalidDiscoveryWindowBounds);
    }
    if window.target_address.trim().is_empty() {
        return Err(PersistenceError::EmptyField {
            field: "window target_address",
        });
    }
    if window.collector_run_id.trim().is_empty() {
        return Err(PersistenceError::EmptyField {
            field: "collector_run_id",
        });
    }
    let target_matches = match (
        observation.event_kind.as_str(),
        observation.key.program,
        observation.market.venue,
        window.target_kind,
    ) {
        (
            "CREATE",
            SourceProgram::Pump,
            soldisco_domain::Venue::PumpBondingCurve,
            WindowTargetKind::PumpMint,
        ) => window.target_address == observation.market.mint,
        (
            "CREATE_POOL",
            SourceProgram::PumpSwap,
            soldisco_domain::Venue::PumpSwap,
            WindowTargetKind::PumpSwapPool,
        ) => window.target_address == observation.market.market_address,
        _ => false,
    };
    if !target_matches {
        return Err(PersistenceError::DiscoveryWindowObservationMismatch {
            window_id: 0,
            field: "opening target",
        });
    }
    if observation.received_time_unix_ms != window.opened_at_unix_ms {
        return Err(PersistenceError::DiscoveryWindowObservationMismatch {
            window_id: 0,
            field: "opened_at_unix_ms",
        });
    }
    Ok(())
}

fn validate_existing_window(
    existing: &WindowIdentityRow,
    observation: &NormalizedObservation,
    window: &NewDiscoveryWindow,
) -> Result<(), PersistenceError> {
    if existing.network != network_name(observation.key.network)
        || existing.source_program != source_program_name(observation.key.program)
        || existing.venue != venue_name(observation.market.venue)
        || existing.market_address != observation.market.market_address
        || existing.mint != observation.market.mint
        || existing.quote_mint != observation.market.quote_mint.as_deref().unwrap_or("")
        || existing.target_kind != window.target_kind.database_name()
        || existing.target_address != window.target_address
    {
        return Err(PersistenceError::DiscoveryWindowIdentityConflict {
            window_id: existing.id,
        });
    }
    Ok(())
}

fn validate_window_membership(
    window: &WindowIdentityRow,
    observation: &NormalizedObservation,
) -> Result<(), PersistenceError> {
    if window.status != "ACTIVE" {
        return Err(PersistenceError::DiscoveryWindowNotActive {
            window_id: window.id,
        });
    }
    for (matches, field) in [
        (
            window.network == network_name(observation.key.network),
            "network",
        ),
        (
            window.source_program == source_program_name(observation.key.program),
            "source_program",
        ),
        (
            window.venue == venue_name(observation.market.venue),
            "venue",
        ),
        (
            window.market_address == observation.market.market_address,
            "market_address",
        ),
        (window.mint == observation.market.mint, "mint"),
        (
            window.quote_mint == observation.market.quote_mint.as_deref().unwrap_or(""),
            "quote_mint",
        ),
        (
            observation.received_time_unix_ms >= window.opened_at_unix_ms
                && observation.received_time_unix_ms < window.closes_at_unix_ms,
            "received_time_unix_ms",
        ),
    ] {
        if !matches {
            return Err(PersistenceError::DiscoveryWindowObservationMismatch {
                window_id: window.id,
                field,
            });
        }
    }
    Ok(())
}

impl WindowIdentityRow {
    fn into_record(
        self,
        observations: Vec<NormalizedObservation>,
    ) -> Result<DiscoveryWindowRecord, PersistenceError> {
        let network = parse_network_value(&self.network)?;
        let source_program = parse_source_program(&self.source_program)?;
        let venue = parse_venue(&self.venue)?;
        let prefilter_values: PrefilterDefaults = serde_json::from_value(self.prefilter_values)?;
        prefilter_values.validate()?;
        let qualification_values: QualificationDefaults =
            serde_json::from_value(self.qualification_values)?;
        qualification_values.validate()?;
        Ok(DiscoveryWindowRecord {
            window_id: self.id,
            market: MarketIdentity {
                network,
                mint: self.mint,
                venue,
                market_address: self.market_address,
                quote_mint: (!self.quote_mint.is_empty()).then_some(self.quote_mint),
            },
            source_program,
            opened_at_unix_ms: self.opened_at_unix_ms,
            closes_at_unix_ms: self.closes_at_unix_ms,
            collector_run_id: self.collector_run_id,
            prefilter_revision: to_u64(self.prefilter_revision, "prefilter_revision")?,
            prefilter_values,
            qualification_revision: to_u64(self.qualification_revision, "qualification_revision")?,
            qualification_values,
            observations,
        })
    }
}

fn parse_network_value(value: &str) -> Result<Network, PersistenceError> {
    parse_network(value)
}
