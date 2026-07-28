use std::time::{Duration, SystemTime, UNIX_EPOCH};

use soldisco_persistence::{Database, PersistenceError, RetentionPolicy, RetentionPruneResult};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Debug)]
pub struct MaintenanceRuntimeConfig {
    pub terminal_history_retention: Duration,
    pub projection_event_retention: Duration,
    pub quarantine_retention: Duration,
    pub interval: Duration,
    pub batch_size: u32,
    pub database_max_bytes: u64,
}

#[derive(Debug, Error)]
pub enum MaintenanceError {
    #[error(transparent)]
    Persistence(#[from] PersistenceError),
    #[error(
        "PostgreSQL uses {current_bytes} bytes, reaching the configured {maximum_bytes}-byte safety limit"
    )]
    StorageLimitReached {
        current_bytes: u64,
        maximum_bytes: u64,
    },
}

pub async fn run_maintenance(
    database: Database,
    config: MaintenanceRuntimeConfig,
    cancellation: CancellationToken,
) -> Result<(), MaintenanceError> {
    let mut interval = tokio::time::interval(config.interval);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        tokio::select! {
            () = cancellation.cancelled() => return Ok(()),
            _ = interval.tick() => {
                run_maintenance_cycle(&database, &config, &cancellation).await?;
            }
        }
    }
}

pub async fn run_maintenance_cycle(
    database: &Database,
    config: &MaintenanceRuntimeConfig,
    cancellation: &CancellationToken,
) -> Result<RetentionPruneResult, MaintenanceError> {
    let pruned = prune_expired_history(database, config, cancellation).await?;
    if pruned.total_rows() > 0 {
        tracing::info!(
            terminal_observations = pruned.terminal_observations,
            projection_events = pruned.projection_events,
            quarantine_records = pruned.quarantine_records,
            "pruned expired discovery history"
        );
    }

    ensure_storage_capacity(database, config.database_max_bytes).await?;
    Ok(pruned)
}

pub async fn ensure_storage_capacity(
    database: &Database,
    maximum_bytes: u64,
) -> Result<u64, MaintenanceError> {
    let current_bytes = database.database_size_bytes().await?;
    if current_bytes >= maximum_bytes {
        return Err(MaintenanceError::StorageLimitReached {
            current_bytes,
            maximum_bytes,
        });
    }
    Ok(current_bytes)
}

async fn prune_expired_history(
    database: &Database,
    config: &MaintenanceRuntimeConfig,
    cancellation: &CancellationToken,
) -> Result<RetentionPruneResult, PersistenceError> {
    let now = unix_time_millis();
    let policy = RetentionPolicy {
        terminal_history_before_unix_ms: cutoff(now, config.terminal_history_retention),
        projection_events_before_unix_ms: cutoff(now, config.projection_event_retention),
        quarantine_before_unix_ms: cutoff(now, config.quarantine_retention),
        batch_size: config.batch_size,
    };
    let mut total = RetentionPruneResult::default();

    loop {
        if cancellation.is_cancelled() {
            return Ok(total);
        }
        let pruned = database.prune_retained_history(policy).await?;
        total.terminal_observations = total
            .terminal_observations
            .saturating_add(pruned.terminal_observations);
        total.projection_events = total
            .projection_events
            .saturating_add(pruned.projection_events);
        total.quarantine_records = total
            .quarantine_records
            .saturating_add(pruned.quarantine_records);

        let maximum_rows = u64::from(config.batch_size);
        if pruned.terminal_observations < maximum_rows
            && pruned.projection_events < maximum_rows
            && pruned.quarantine_records < maximum_rows
        {
            return Ok(total);
        }
        tokio::task::yield_now().await;
    }
}

fn cutoff(now_unix_ms: i64, retention: Duration) -> i64 {
    let retention_ms = i64::try_from(retention.as_millis()).unwrap_or(i64::MAX);
    now_unix_ms.saturating_sub(retention_ms)
}

fn unix_time_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::cutoff;

    #[test]
    fn retention_cutoff_saturates_instead_of_wrapping() {
        assert_eq!(cutoff(1_000, Duration::from_secs(1)), 0);
        assert_eq!(cutoff(i64::MIN, Duration::from_secs(1)), i64::MIN);
    }
}
