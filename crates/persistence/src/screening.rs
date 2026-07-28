use serde::{Deserialize, Serialize};
use serde_json::Value;
use soldisco_domain::{AssessmentDecision, RuleResult, Score, Venue};

use crate::{
    Database, PersistenceError,
    values::{assessment_decision_name, require_non_empty, to_i64, venue_name},
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ScreeningRun {
    pub observation_work_id: Option<i64>,
    pub mint: String,
    pub venue: Venue,
    pub market_address: String,
    pub ruleset_version: String,
    pub decision: AssessmentDecision,
    pub risk_score: Option<Score>,
    pub opportunity_score: Option<Score>,
    pub observed_slot: Option<u64>,
    pub rules: Vec<RuleResult>,
}

impl Database {
    /// Persists an immutable screening audit and all rule results atomically.
    ///
    /// A work-linked run is idempotent when its complete serialized input is
    /// identical. Reusing the same work id for a different assessment fails.
    pub async fn save_screening_run(&self, run: &ScreeningRun) -> Result<i64, PersistenceError> {
        validate_screening_run(run)?;
        let observed_slot = run
            .observed_slot
            .map(|slot| to_i64(slot, "screening observed_slot"))
            .transpose()?;
        let screening_record: Value = serde_json::to_value(run)?;

        let mut transaction = self.pool.begin().await?;
        let inserted_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO screening_runs (\
                observation_work_id, mint, venue, market_address, \
                ruleset_version, decision, risk_score, opportunity_score, \
                observed_slot, screening_record\
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
             ON CONFLICT (observation_work_id) \
                WHERE observation_work_id IS NOT NULL \
             DO NOTHING \
             RETURNING id",
        )
        .bind(run.observation_work_id)
        .bind(&run.mint)
        .bind(venue_name(run.venue))
        .bind(&run.market_address)
        .bind(&run.ruleset_version)
        .bind(assessment_decision_name(run.decision))
        .bind(run.risk_score.map(Score::value).map(i16::from))
        .bind(run.opportunity_score.map(Score::value).map(i16::from))
        .bind(observed_slot)
        .bind(&screening_record)
        .fetch_optional(&mut *transaction)
        .await?;

        let run_id = match inserted_id {
            Some(run_id) => {
                for rule in &run.rules {
                    let rule_slot = rule
                        .observed_slot
                        .map(|slot| to_i64(slot, "rule observed_slot"))
                        .transpose()?;
                    sqlx::query(
                        "INSERT INTO screening_rule_results (\
                            screening_run_id, rule_id, rule_version, decision, \
                            reason_code, evidence_reference, observed_slot\
                         ) VALUES ($1, $2, $3, $4, $5, $6, $7)",
                    )
                    .bind(run_id)
                    .bind(&rule.rule_id)
                    .bind(&rule.rule_version)
                    .bind(assessment_decision_name(rule.decision))
                    .bind(&rule.reason_code)
                    .bind(&rule.evidence_reference)
                    .bind(rule_slot)
                    .execute(&mut *transaction)
                    .await?;
                }
                run_id
            }
            None => {
                let work_id =
                    run.observation_work_id
                        .ok_or(PersistenceError::InvalidStoredValue {
                            field: "screening_runs.observation_work_id",
                            value: "NULL conflict".to_owned(),
                        })?;
                let existing = sqlx::query_as::<_, (i64, Value)>(
                    "SELECT id, screening_record \
                     FROM screening_runs \
                     WHERE observation_work_id = $1",
                )
                .bind(work_id)
                .fetch_one(&mut *transaction)
                .await?;
                if existing.1 != screening_record {
                    return Err(PersistenceError::ScreeningRunConflict { work_id });
                }
                existing.0
            }
        };

        transaction.commit().await?;
        Ok(run_id)
    }
}

fn validate_screening_run(run: &ScreeningRun) -> Result<(), PersistenceError> {
    require_non_empty(&run.mint, "screening mint")?;
    require_non_empty(&run.market_address, "screening market_address")?;
    require_non_empty(&run.ruleset_version, "ruleset_version")?;

    for rule in &run.rules {
        require_non_empty(&rule.rule_id, "rule_id")?;
        require_non_empty(&rule.rule_version, "rule_version")?;
        require_non_empty(&rule.reason_code, "reason_code")?;
    }

    let expected = if run
        .rules
        .iter()
        .any(|rule| rule.decision == AssessmentDecision::Reject)
    {
        AssessmentDecision::Reject
    } else if run
        .rules
        .iter()
        .any(|rule| rule.decision == AssessmentDecision::Unknown)
        || run.rules.is_empty()
    {
        AssessmentDecision::Unknown
    } else {
        AssessmentDecision::Pass
    };

    if run.decision == AssessmentDecision::Pass && run.rules.is_empty() {
        return Err(PersistenceError::EmptyPassingRules);
    }
    if expected != run.decision {
        return Err(PersistenceError::ScreeningDecisionMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use soldisco_domain::{AssessmentDecision, RuleResult, Venue};

    use super::{ScreeningRun, validate_screening_run};
    use crate::PersistenceError;

    fn run(decision: AssessmentDecision, rules: Vec<RuleResult>) -> ScreeningRun {
        ScreeningRun {
            observation_work_id: Some(1),
            mint: "mint".to_owned(),
            venue: Venue::PumpBondingCurve,
            market_address: "curve".to_owned(),
            ruleset_version: "1".to_owned(),
            decision,
            risk_score: None,
            opportunity_score: None,
            observed_slot: Some(1),
            rules,
        }
    }

    #[test]
    fn pass_cannot_be_persisted_without_an_explicit_passing_rule() {
        let error = validate_screening_run(&run(AssessmentDecision::Pass, Vec::new()))
            .expect_err("empty rules are UNKNOWN, never PASS");

        assert!(matches!(error, PersistenceError::EmptyPassingRules));
    }

    #[test]
    fn aggregate_decision_must_match_rules() {
        let error = validate_screening_run(&run(
            AssessmentDecision::Pass,
            vec![RuleResult::unknown("evidence", "1", "EVIDENCE_MISSING")],
        ))
        .expect_err("unknown evidence cannot be persisted as pass");

        assert!(matches!(error, PersistenceError::ScreeningDecisionMismatch));
    }
}
