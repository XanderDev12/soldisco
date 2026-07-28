use std::{collections::VecDeque, time::Duration};

use soldisco_discovery_engine::{ObservationWindowStatus, WindowIncompleteReason};

use super::{collector::ProgramLogBatch, intake::BatchPurpose};

pub enum PendingActivityAdmission {
    Ready(ProgramLogBatch),
    Deferred,
    Dropped,
}

/// Bounded holding area for activity that arrived before its provisional
/// discovery window resolved.
pub struct PendingActivityQueue {
    capacity: usize,
    maximum_hold: Duration,
    batches: VecDeque<PendingActivityBatch>,
}

struct PendingActivityBatch {
    batch: ProgramLogBatch,
    release_at_unix_ms: i64,
}

impl PendingActivityQueue {
    #[must_use]
    pub fn new(capacity: usize, maximum_hold: Duration) -> Self {
        assert!(capacity > 0, "pending-activity capacity must be positive");
        assert!(
            !maximum_hold.is_zero(),
            "pending-activity hold must be positive"
        );
        Self {
            capacity,
            maximum_hold,
            batches: VecDeque::with_capacity(capacity),
        }
    }

    pub fn admit(&mut self, batch: ProgramLogBatch, now_unix_ms: i64) -> PendingActivityAdmission {
        if batch.purpose != BatchPurpose::TrackedActivity || !has_pending_token(&batch) {
            return PendingActivityAdmission::Ready(batch);
        }
        if self.batches.len() >= self.capacity {
            for token in &batch.window_tokens {
                token.cancel_with_reason(WindowIncompleteReason::QueueOverflow);
            }
            return PendingActivityAdmission::Dropped;
        }
        let maximum_hold_ms = i64::try_from(self.maximum_hold.as_millis()).unwrap_or(i64::MAX);
        self.batches.push_back(PendingActivityBatch {
            batch,
            release_at_unix_ms: now_unix_ms.saturating_add(maximum_hold_ms),
        });
        PendingActivityAdmission::Deferred
    }

    pub fn drain_resolved(&mut self, now_unix_ms: i64) -> Vec<ProgramLogBatch> {
        let mut ready = Vec::new();
        let pending_count = self.batches.len();
        for _ in 0..pending_count {
            let Some(pending) = self.batches.pop_front() else {
                break;
            };
            if has_pending_token(&pending.batch) && now_unix_ms < pending.release_at_unix_ms {
                self.batches.push_back(pending);
            } else {
                ready.push(pending.batch);
            }
        }
        ready
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.batches.len()
    }
}

fn has_pending_token(batch: &ProgramLogBatch) -> bool {
    batch
        .window_tokens
        .iter()
        .any(|token| token.status() == ObservationWindowStatus::Pending)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use soldisco_discovery_engine::{
        ObservationTarget, ObservationWindowRegistry, ObservationWindowStatus,
    };

    use super::{PendingActivityAdmission, PendingActivityQueue};
    use crate::jobs::{collector::ProgramLogBatch, intake::BatchPurpose};

    fn activity_batch(token: soldisco_discovery_engine::ObservationWindowToken) -> ProgramLogBatch {
        ProgramLogBatch {
            purpose: BatchPurpose::TrackedActivity,
            slot: 42,
            transaction_index: None,
            signature: "activity".to_owned(),
            received_time_unix_ms: 1_100,
            instructions: Vec::new(),
            log_messages: Vec::new(),
            transaction_error: None,
            window_tokens: vec![token],
            processing_succeeded: false,
        }
    }

    #[test]
    fn pending_activity_is_released_after_confirmation() {
        let windows = ObservationWindowRegistry::new(2);
        let provision = windows.provision_target(
            ObservationTarget::PumpMint("mint".to_owned()),
            "mint".to_owned(),
            1_000,
            Duration::from_secs(1),
        );
        let mut pending = PendingActivityQueue::new(2, Duration::from_secs(2));

        assert!(matches!(
            pending.admit(activity_batch(provision.token.clone()), 1_100),
            PendingActivityAdmission::Deferred
        ));
        assert_eq!(pending.len(), 1);
        assert!(provision.token.confirm());

        assert_eq!(pending.drain_resolved(1_200).len(), 1);
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn observation_close_does_not_cancel_a_pending_discovery_token() {
        let windows = ObservationWindowRegistry::new(2);
        let provision = windows.provision_target(
            ObservationTarget::PumpMint("mint".to_owned()),
            "mint".to_owned(),
            1_000,
            Duration::from_millis(500),
        );
        let mut pending = PendingActivityQueue::new(2, Duration::from_secs(2));
        assert!(matches!(
            pending.admit(activity_batch(provision.token.clone()), 1_100),
            PendingActivityAdmission::Deferred
        ));

        assert!(pending.drain_resolved(1_500).is_empty());
        assert_eq!(provision.token.status(), ObservationWindowStatus::Pending);
        assert_eq!(pending.drain_resolved(3_100).len(), 1);
        assert_eq!(pending.len(), 0);
        assert_eq!(
            provision.token.status(),
            ObservationWindowStatus::Pending,
            "holding expiry must not cancel the discovery globally"
        );
    }

    #[test]
    fn cancelled_activity_is_released_immediately() {
        let windows = ObservationWindowRegistry::new(2);
        let provision = windows.provision_target(
            ObservationTarget::PumpMint("mint".to_owned()),
            "mint".to_owned(),
            1_000,
            Duration::from_secs(5),
        );
        let mut pending = PendingActivityQueue::new(2, Duration::from_secs(2));
        assert!(matches!(
            pending.admit(activity_batch(provision.token.clone()), 1_100),
            PendingActivityAdmission::Deferred
        ));
        provision.token.cancel_if_pending();

        assert_eq!(pending.drain_resolved(1_200).len(), 1);
        assert_eq!(pending.len(), 0);
    }

    #[test]
    fn full_queue_drops_the_newest_provisional_batch() {
        let windows = ObservationWindowRegistry::new(2);
        let first = windows.provision_target(
            ObservationTarget::PumpMint("first".to_owned()),
            "first".to_owned(),
            1_000,
            Duration::from_secs(5),
        );
        let second = windows.provision_target(
            ObservationTarget::PumpMint("second".to_owned()),
            "second".to_owned(),
            1_000,
            Duration::from_secs(5),
        );
        let mut pending = PendingActivityQueue::new(1, Duration::from_secs(2));

        assert!(matches!(
            pending.admit(activity_batch(first.token), 1_100),
            PendingActivityAdmission::Deferred
        ));
        assert!(matches!(
            pending.admit(activity_batch(second.token), 1_100),
            PendingActivityAdmission::Dropped
        ));
        assert_eq!(pending.len(), 1);
    }
}
