use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex as StdMutex},
    time::Duration,
};

use tokio::{
    sync::{AcquireError, Mutex, OwnedSemaphorePermit, Semaphore},
    time::{Instant, sleep_until},
};

/// Shared admission control for every Pump and PumpSwap discovery read.
///
/// Concurrency protects local resources, pacing protects provider request-rate
/// limits, and the freshness-retained signature set prevents duplicate
/// subscriptions from turning one discovery transaction into multiple HTTP
/// attempts.
#[derive(Clone)]
pub struct DiscoveryRpcGate {
    inner: Arc<DiscoveryRpcGateInner>,
}

struct DiscoveryRpcGateInner {
    permits: Arc<Semaphore>,
    pacing: Mutex<PacingState>,
    request_interval: Duration,
    rate_limit_cooldown: Duration,
    signatures: StdMutex<RecentSignatures>,
}

#[derive(Default)]
struct PacingState {
    next_allowed: Option<Instant>,
    cooldown_until: Option<Instant>,
}

struct RecentSignatures {
    retention: Duration,
    expirations: VecDeque<(Instant, String)>,
    values: HashMap<String, Instant>,
}

impl DiscoveryRpcGate {
    #[must_use]
    pub fn new(
        maximum_in_flight: usize,
        requests_per_second: u32,
        rate_limit_cooldown: Duration,
        signature_retention: Duration,
    ) -> Self {
        assert!(maximum_in_flight > 0, "RPC concurrency must be positive");
        assert!(requests_per_second > 0, "RPC request rate must be positive");
        assert!(
            !signature_retention.is_zero(),
            "signature retention must be positive"
        );
        let request_interval = Duration::from_secs(1) / requests_per_second.min(1_000_000_000);
        Self {
            inner: Arc::new(DiscoveryRpcGateInner {
                permits: Arc::new(Semaphore::new(maximum_in_flight)),
                pacing: Mutex::new(PacingState::default()),
                request_interval,
                rate_limit_cooldown,
                signatures: StdMutex::new(RecentSignatures {
                    retention: signature_retention,
                    expirations: VecDeque::new(),
                    values: HashMap::new(),
                }),
            }),
        }
    }

    /// Claims a signature once while it could still pass the freshness filter.
    ///
    /// Expired claims can be removed safely because the source timestamp makes
    /// the corresponding discovery ineligible for another HTTP request.
    #[must_use]
    pub fn claim_signature(&self, signature: &str) -> bool {
        let mut signatures = self
            .inner
            .signatures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        signatures.claim_at(signature, Instant::now())
    }

    /// Acquires one shared concurrency permit, then waits for the next global
    /// request-start slot. This does not perform or retry an RPC request.
    pub async fn acquire(&self) -> Result<OwnedSemaphorePermit, AcquireError> {
        let permit = self.inner.permits.clone().acquire_owned().await?;
        loop {
            let wait_until = {
                let mut pacing = self.inner.pacing.lock().await;
                pacing.reserve_if_ready(Instant::now(), self.inner.request_interval)
            };
            match wait_until {
                None => break,
                Some(wait_until) => sleep_until(wait_until).await,
            }
        }
        Ok(permit)
    }

    /// Delays later, distinct signatures after a provider rate-limit response.
    /// The failed signature itself is never retried.
    pub async fn record_rate_limit(&self) {
        let mut pacing = self.inner.pacing.lock().await;
        let cooldown_until = Instant::now() + self.inner.rate_limit_cooldown;
        pacing.cooldown_until = Some(
            pacing
                .cooldown_until
                .map_or(cooldown_until, |current| current.max(cooldown_until)),
        );
    }

    #[must_use]
    pub fn rate_limit_cooldown(&self) -> Duration {
        self.inner.rate_limit_cooldown
    }
}

impl RecentSignatures {
    fn claim_at(&mut self, signature: &str, now: Instant) -> bool {
        while let Some((expires_at, signature)) = self.expirations.front() {
            if *expires_at > now {
                break;
            }
            if self
                .values
                .get(signature)
                .is_some_and(|stored| stored == expires_at)
            {
                self.values.remove(signature);
            }
            self.expirations.pop_front();
        }
        if self.values.contains_key(signature) {
            return false;
        }
        let expires_at = now + self.retention;
        self.values.insert(signature.to_owned(), expires_at);
        self.expirations
            .push_back((expires_at, signature.to_owned()));
        true
    }
}

impl PacingState {
    fn reserve_if_ready(&mut self, now: Instant, interval: Duration) -> Option<Instant> {
        let ready_at = self
            .next_allowed
            .into_iter()
            .chain(self.cooldown_until)
            .max()
            .unwrap_or(now);
        if ready_at > now {
            return Some(ready_at);
        }
        self.next_allowed = Some(now + interval);
        if self.cooldown_until.is_some_and(|cooldown| cooldown <= now) {
            self.cooldown_until = None;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::Instant;

    use super::{DiscoveryRpcGate, PacingState, RecentSignatures};

    #[test]
    fn signatures_remain_claimed_for_the_full_freshness_horizon() {
        let now = Instant::now();
        let mut signatures = RecentSignatures {
            retention: Duration::from_secs(30),
            expirations: std::collections::VecDeque::new(),
            values: std::collections::HashMap::new(),
        };

        assert!(signatures.claim_at("first", now));
        assert!(!signatures.claim_at("first", now + Duration::from_secs(29)));
        assert!(signatures.claim_at("first", now + Duration::from_secs(30)));
    }

    #[test]
    fn pacing_reserves_only_one_request_per_interval() {
        let now = Instant::now();
        let interval = Duration::from_millis(100);
        let mut pacing = PacingState::default();

        assert_eq!(pacing.reserve_if_ready(now, interval), None);
        assert_eq!(pacing.reserve_if_ready(now, interval), Some(now + interval));
        assert_eq!(pacing.reserve_if_ready(now + interval, interval), None);
    }

    #[tokio::test]
    async fn rate_limit_cooldown_moves_the_next_request_later() {
        let gate = DiscoveryRpcGate::new(1, 100, Duration::from_secs(1), Duration::from_secs(30));
        let _permit = gate.acquire().await.expect("first permit");
        gate.record_rate_limit().await;

        let pacing = gate.inner.pacing.lock().await;
        let cooldown = pacing.cooldown_until.expect("cooldown");
        assert!(cooldown > Instant::now() + Duration::from_millis(900));
    }

    #[tokio::test]
    async fn semaphore_waiters_remain_spaced_at_actual_request_start() {
        let gate = DiscoveryRpcGate::new(1, 20, Duration::from_secs(1), Duration::from_secs(30));
        let first_permit = gate.acquire().await.expect("first permit");
        let first_gate = gate.clone();
        let second_gate = gate.clone();
        let first_waiter = tokio::spawn(async move {
            let permit = first_gate.acquire().await.expect("first waiter permit");
            let admitted = Instant::now();
            drop(permit);
            admitted
        });
        let second_waiter = tokio::spawn(async move {
            let permit = second_gate.acquire().await.expect("second waiter permit");
            let admitted = Instant::now();
            drop(permit);
            admitted
        });

        tokio::time::sleep(Duration::from_millis(125)).await;
        drop(first_permit);

        let first = first_waiter.await.expect("first waiter task");
        let second = second_waiter.await.expect("second waiter task");
        let spacing = first.max(second) - first.min(second);

        assert!(
            spacing >= Duration::from_millis(45),
            "spacing was {spacing:?}"
        );
    }
}
