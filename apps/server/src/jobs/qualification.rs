use std::{collections::HashMap, time::Duration};

use soldisco_discovery_engine::{ObservationWindowRegistry, WindowIncompleteReason};
use soldisco_persistence::{
    Database, DiscoveryWindowProjectionReadiness, PersistenceError,
    build_discovery_window_finalization,
};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

use crate::state::LiveEventBus;

const FINALIZER_POLL_INTERVAL: Duration = Duration::from_millis(100);
const FINALIZER_RETRY_DELAY: Duration = Duration::from_secs(1);
const ABANDONED_WINDOW_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const SHUTDOWN_DRAIN_LIMIT: Duration = Duration::from_secs(2);

#[derive(Default)]
struct FinalizationRetrySchedule {
    retry_at: HashMap<i64, tokio::time::Instant>,
}

impl FinalizationRetrySchedule {
    fn is_due(&self, window_id: i64, now: tokio::time::Instant) -> bool {
        self.retry_at
            .get(&window_id)
            .is_none_or(|retry_at| *retry_at <= now)
    }

    fn defer(&mut self, window_id: i64, now: tokio::time::Instant, delay: Duration) {
        self.retry_at.insert(window_id, now + delay);
    }

    fn clear(&mut self, window_id: i64) {
        self.retry_at.remove(&window_id);
    }
}

#[derive(Debug, Error)]
pub enum QualificationWorkerError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error("durable window id cannot be represented by the browser contract")]
    WindowIdOutOfRange,
}

pub async fn finalize_abandoned_windows(
    database: &Database,
    events: &LiveEventBus,
    current_collector_run_id: &str,
) -> Result<usize, QualificationWorkerError> {
    let window_ids = database
        .load_interrupted_discovery_window_ids(current_collector_run_id)
        .await?;
    let mut finalized = 0;
    for window_id in window_ids {
        if attempt_finalize_window(
            database,
            events,
            window_id,
            Some(WindowIncompleteReason::PipelineRestarted),
        )
        .await?
        .is_some_and(|committed| committed)
        {
            finalized += 1;
        }
    }
    Ok(finalized)
}

pub async fn run_qualification_finalizer(
    database: Database,
    events: LiveEventBus,
    windows: ObservationWindowRegistry,
    collector_run_id: String,
    cancellation: CancellationToken,
) -> Result<(), QualificationWorkerError> {
    let mut tick = tokio::time::interval(FINALIZER_POLL_INTERVAL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut abandoned_tick = tokio::time::interval(ABANDONED_WINDOW_RETRY_INTERVAL);
    abandoned_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut retries = FinalizationRetrySchedule::default();

    loop {
        tokio::select! {
            () = cancellation.cancelled() => {
                windows.cancel_all_confirmed(WindowIncompleteReason::StreamStopped);
                drain_shutdown(
                    &database,
                    &events,
                    &windows,
                    &collector_run_id,
                ).await?;
                return Ok(());
            }
            _ = tick.tick() => {
                finalize_ready_tokens(
                    &database,
                    &events,
                    &windows,
                    &mut retries,
                    FINALIZER_RETRY_DELAY,
                ).await?;
            }
            _ = abandoned_tick.tick() => {
                let abandoned = finalize_abandoned_windows(
                    &database,
                    &events,
                    &collector_run_id,
                ).await?;
                if abandoned > 0 {
                    tracing::warn!(
                        windows = abandoned,
                        "finalized interrupted discovery windows as incomplete"
                    );
                }
            }
        }
    }
}

async fn finalize_ready_tokens(
    database: &Database,
    events: &LiveEventBus,
    windows: &ObservationWindowRegistry,
    retries: &mut FinalizationRetrySchedule,
    retry_delay: Duration,
) -> Result<usize, QualificationWorkerError> {
    let now = unix_time_millis();
    let candidates = windows.finalization_candidates(now);
    let mut finalized = 0;
    for token in candidates {
        let window_id = token
            .durable_window_id()
            .ok_or(QualificationWorkerError::WindowIdOutOfRange)?;
        let retry_now = tokio::time::Instant::now();
        if !retries.is_due(window_id, retry_now) {
            continue;
        }
        let Some(claim) = windows.claim_finalization(&token) else {
            continue;
        };
        let Some(committed) =
            attempt_finalize_window(database, events, window_id, claim.incomplete_reason()).await?
        else {
            retries.defer(window_id, tokio::time::Instant::now(), retry_delay);
            continue;
        };
        retries.clear(window_id);
        if committed {
            finalized += 1;
        }
        let _ = claim.mark_committed();
    }
    windows.cleanup_finalized();
    Ok(finalized)
}

async fn drain_shutdown(
    database: &Database,
    events: &LiveEventBus,
    windows: &ObservationWindowRegistry,
    collector_run_id: &str,
) -> Result<(), QualificationWorkerError> {
    let deadline = tokio::time::Instant::now() + SHUTDOWN_DRAIN_LIMIT;
    let mut retries = FinalizationRetrySchedule::default();
    while tokio::time::Instant::now() < deadline {
        let _ =
            finalize_ready_tokens(database, events, windows, &mut retries, Duration::ZERO).await?;
        if database
            .load_active_discovery_window_ids(Some(collector_run_id))
            .await?
            .is_empty()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }

    // Any processor work still outstanding is being dropped by the cancelled
    // pipeline. The durable observations that did commit remain useful, but
    // the window is explicitly incomplete.
    for window_id in database
        .load_active_discovery_window_ids(Some(collector_run_id))
        .await?
    {
        let _ = attempt_finalize_window(
            database,
            events,
            window_id,
            Some(WindowIncompleteReason::StreamStopped),
        )
        .await?;
    }
    Ok(())
}

/// A discovery window can close before its queued projection work commits,
/// especially across a process restart. That is recoverable lag, not a
/// pipeline failure: leave the window active and retry on the next tick.
async fn attempt_finalize_window(
    database: &Database,
    events: &LiveEventBus,
    window_id: i64,
    incomplete_reason: Option<WindowIncompleteReason>,
) -> Result<Option<bool>, QualificationWorkerError> {
    let readiness = database
        .load_discovery_window_projection_readiness(window_id)
        .await?;
    if readiness == DiscoveryWindowProjectionReadiness::Pending {
        tracing::debug!(
            window_id,
            "qualification window is waiting for durable discovery projection work"
        );
        return Ok(None);
    }
    let incomplete_reason = terminal_incomplete_reason(readiness, incomplete_reason);

    match finalize_window(database, events, window_id, incomplete_reason).await {
        Ok(committed) => Ok(Some(committed)),
        Err(QualificationWorkerError::Persistence(
            PersistenceError::DiscoveryWindowProjectionMissing { .. }
            | PersistenceError::DiscoveryWindowEvidenceChanged { .. },
        )) => {
            tracing::debug!(
                window_id,
                "qualification window is waiting for durable evidence to settle"
            );
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

fn terminal_incomplete_reason(
    readiness: DiscoveryWindowProjectionReadiness,
    incomplete_reason: Option<WindowIncompleteReason>,
) -> Option<WindowIncompleteReason> {
    if readiness == DiscoveryWindowProjectionReadiness::OpeningProjectionUnavailable {
        Some(WindowIncompleteReason::ProcessingFailed)
    } else {
        incomplete_reason
    }
}

async fn finalize_window(
    database: &Database,
    events: &LiveEventBus,
    window_id: i64,
    incomplete_reason: Option<WindowIncompleteReason>,
) -> Result<bool, QualificationWorkerError> {
    let window = database.load_discovery_window(window_id).await?;
    let finalized_at_unix_ms = unix_time_millis();
    let finalized = build_discovery_window_finalization(
        &window,
        finalized_at_unix_ms,
        incomplete_reason.map(WindowIncompleteReason::as_str),
    )?;
    let committed = database.finalize_discovery_window(&finalized).await?;
    if committed {
        events.publish_discovery_projection_changed();
        tracing::info!(
            window_id,
            mint = %window.market.mint,
            decision = ?finalized.decision,
            trades = finalized.summary.trades,
            unique_traders = finalized.summary.unique_traders,
            "finalized durable discovery qualification window"
        );
    }
    Ok(committed)
}

fn unix_time_millis() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use soldisco_discovery_engine::WindowIncompleteReason;
    use soldisco_persistence::DiscoveryWindowProjectionReadiness;

    use super::{FinalizationRetrySchedule, terminal_incomplete_reason};

    #[test]
    fn retry_schedule_delays_only_the_window_that_needs_to_settle() {
        let mut retries = FinalizationRetrySchedule::default();
        let now = tokio::time::Instant::now();

        retries.defer(7, now, Duration::from_secs(1));
        retries.defer(7, now, Duration::from_secs(2));

        assert!(!retries.is_due(7, now));
        assert!(retries.is_due(8, now));
        assert_eq!(
            retries.retry_at.len(),
            1,
            "repeated lag keeps one bounded retry entry per window"
        );
        assert!(!retries.is_due(7, now + Duration::from_millis(1_999)));
        assert!(retries.is_due(7, now + Duration::from_secs(2)));

        retries.clear(7);
        assert!(retries.is_due(7, now));
    }

    #[test]
    fn terminally_unavailable_opening_projection_forces_processing_failure() {
        assert_eq!(
            terminal_incomplete_reason(
                DiscoveryWindowProjectionReadiness::OpeningProjectionUnavailable,
                None,
            ),
            Some(WindowIncompleteReason::ProcessingFailed)
        );
        assert_eq!(
            terminal_incomplete_reason(
                DiscoveryWindowProjectionReadiness::OpeningProjectionUnavailable,
                Some(WindowIncompleteReason::PipelineRestarted),
            ),
            Some(WindowIncompleteReason::ProcessingFailed)
        );
        assert_eq!(
            terminal_incomplete_reason(
                DiscoveryWindowProjectionReadiness::Ready,
                Some(WindowIncompleteReason::StreamStopped),
            ),
            Some(WindowIncompleteReason::StreamStopped)
        );
    }
}
