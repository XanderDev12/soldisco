use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
    time::Duration,
};

use serde::Serialize;
pub use soldisco_domain::TradeSide;
use soldisco_domain::{MarketIdentity, Venue};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObservationTarget {
    PumpMint(String),
    PumpSwapPool(String),
}

impl ObservationTarget {
    #[must_use]
    pub fn for_market(market: &MarketIdentity) -> Option<Self> {
        match market.venue {
            Venue::PumpBondingCurve => Some(Self::PumpMint(market.mint.clone())),
            Venue::PumpSwap => Some(Self::PumpSwapPool(market.market_address.clone())),
            Venue::RaydiumCpmm | Venue::RaydiumClmm | Venue::RaydiumAmmV4 => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObservationWindow {
    pub target: ObservationTarget,
    pub mint: String,
    pub opened_at_unix_ms: i64,
    pub closes_at_unix_ms: i64,
}

const WINDOW_PENDING: u8 = 0;
const WINDOW_CONFIRMED: u8 = 1;
const WINDOW_CANCELLED: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObservationWindowStatus {
    Pending,
    Confirmed,
    Cancelled,
}

/// A stable reference to one exact observation-window generation.
///
/// Tokens travel with queued activity so processing delay cannot change the
/// original receipt-time decision. A provisional token becomes confirmed only
/// after its authoritative discovery transaction has normalized successfully.
#[derive(Clone, Debug)]
pub struct ObservationWindowToken {
    generation: u64,
    window: ObservationWindow,
    state: Arc<AtomicU8>,
}

impl ObservationWindowToken {
    #[must_use]
    pub fn target(&self) -> &ObservationTarget {
        &self.window.target
    }

    #[must_use]
    pub fn snapshot(&self) -> &ObservationWindow {
        &self.window
    }

    #[must_use]
    pub fn is_open_at(&self, observed_at_unix_ms: i64) -> bool {
        self.state.load(Ordering::Acquire) != WINDOW_CANCELLED
            && self.window.opened_at_unix_ms <= observed_at_unix_ms
            && observed_at_unix_ms < self.window.closes_at_unix_ms
    }

    #[must_use]
    pub fn is_confirmed_at(&self, observed_at_unix_ms: i64) -> bool {
        self.state.load(Ordering::Acquire) == WINDOW_CONFIRMED
            && self.window.opened_at_unix_ms <= observed_at_unix_ms
            && observed_at_unix_ms < self.window.closes_at_unix_ms
    }

    #[must_use]
    pub fn status(&self) -> ObservationWindowStatus {
        match self.state.load(Ordering::Acquire) {
            WINDOW_PENDING => ObservationWindowStatus::Pending,
            WINDOW_CONFIRMED => ObservationWindowStatus::Confirmed,
            _ => ObservationWindowStatus::Cancelled,
        }
    }

    /// Confirms a provisional generation. Cancelled generations stay closed.
    #[must_use]
    pub fn confirm(&self) -> bool {
        match self.state.compare_exchange(
            WINDOW_PENDING,
            WINDOW_CONFIRMED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => true,
            Err(WINDOW_CONFIRMED) => true,
            Err(_) => false,
        }
    }

    pub fn cancel_if_pending(&self) {
        let _ = self.state.compare_exchange(
            WINDOW_PENDING,
            WINDOW_CANCELLED,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    fn cancel(&self) {
        self.state.store(WINDOW_CANCELLED, Ordering::Release);
    }
}

impl PartialEq for ObservationWindowToken {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation && self.window == other.window
    }
}

impl Eq for ObservationWindowToken {}

#[derive(Clone, Debug)]
pub struct ObservationWindowProvision {
    pub token: ObservationWindowToken,
    pub newly_opened: bool,
}

#[derive(Default)]
struct ObservationWindowState {
    next_generation: u64,
    windows: HashMap<ObservationTarget, ObservationWindowToken>,
}

#[derive(Clone)]
pub struct ObservationWindowRegistry {
    state: Arc<Mutex<ObservationWindowState>>,
    maximum_active: usize,
}

impl ObservationWindowRegistry {
    #[must_use]
    pub fn new(maximum_active: usize) -> Self {
        assert!(
            maximum_active > 0,
            "observation-window capacity must be positive"
        );
        Self {
            state: Arc::new(Mutex::new(ObservationWindowState::default())),
            maximum_active,
        }
    }

    /// Opens a provisional, non-extending window as soon as a fresh discovery
    /// log is admitted. This closes the race between the discovery HTTP read
    /// and immediately following market activity.
    #[must_use]
    pub fn provision_target(
        &self,
        target: ObservationTarget,
        mint: String,
        opened_at_unix_ms: i64,
        duration: Duration,
    ) -> ObservationWindowProvision {
        let duration_ms = i64::try_from(duration.as_millis()).unwrap_or(i64::MAX);
        let closes_at_unix_ms = opened_at_unix_ms.saturating_add(duration_ms);
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.windows.retain(|_, token| {
            token.state.load(Ordering::Acquire) != WINDOW_CANCELLED
                && token.window.closes_at_unix_ms > opened_at_unix_ms
        });

        if let Some(existing) = state
            .windows
            .get(&target)
            .filter(|token| token.is_open_at(opened_at_unix_ms))
        {
            return ObservationWindowProvision {
                token: existing.clone(),
                newly_opened: false,
            };
        }
        if state.windows.len() >= self.maximum_active
            && let Some(evicted) = state
                .windows
                .iter()
                .min_by_key(|(target, token)| (token.window.closes_at_unix_ms, (*target).clone()))
                .map(|(target, _)| target.clone())
            && let Some(token) = state.windows.remove(&evicted)
        {
            token.cancel();
        }

        let generation = state.next_generation;
        state.next_generation = state.next_generation.wrapping_add(1);
        let token = ObservationWindowToken {
            generation,
            window: ObservationWindow {
                target: target.clone(),
                mint,
                opened_at_unix_ms,
                closes_at_unix_ms,
            },
            state: Arc::new(AtomicU8::new(WINDOW_PENDING)),
        };
        state.windows.insert(target, token.clone());
        ObservationWindowProvision {
            token,
            newly_opened: true,
        }
    }

    /// Opens one non-extending window for a newly discovered market.
    ///
    /// Replayed discovery events do not keep a candidate alive indefinitely.
    /// If capacity is reached, the window that closes soonest is evicted.
    #[must_use]
    pub fn open_market(
        &self,
        market: &MarketIdentity,
        opened_at_unix_ms: i64,
        duration: Duration,
    ) -> Option<ObservationWindow> {
        let target = ObservationTarget::for_market(market)?;
        let provision =
            self.provision_target(target, market.mint.clone(), opened_at_unix_ms, duration);
        let _ = provision.token.confirm();
        Some(provision.token.snapshot().clone())
    }

    #[must_use]
    pub fn matching_token(
        &self,
        target: &ObservationTarget,
        observed_at_unix_ms: i64,
    ) -> Option<ObservationWindowToken> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .windows
            .get(target)
            .filter(|token| token.is_open_at(observed_at_unix_ms))
            .cloned()
    }

    #[must_use]
    pub fn is_active(&self, target: &ObservationTarget, now_unix_ms: i64) -> bool {
        self.matching_token(target, now_unix_ms).is_some()
    }

    #[must_use]
    pub fn active_count(&self, now_unix_ms: i64) -> usize {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .windows
            .retain(|_, token| token.is_open_at(now_unix_ms));
        state.windows.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TradeObservation {
    pub side: TradeSide,
    pub wallet: String,
    pub base_units: u128,
    pub quote_units: u128,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RollingMarketMetrics {
    pub trades: u64,
    pub buys: u64,
    pub sells: u64,
    pub buy_quote_units: u128,
    pub sell_quote_units: u128,
    pub unique_traders: u64,
    #[serde(skip)]
    wallets: HashSet<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MetricsError {
    Overflow,
}

impl RollingMarketMetrics {
    pub fn record(&mut self, trade: &TradeObservation) -> Result<(), MetricsError> {
        self.trades = self.trades.checked_add(1).ok_or(MetricsError::Overflow)?;
        self.wallets.insert(trade.wallet.clone());
        self.unique_traders =
            u64::try_from(self.wallets.len()).map_err(|_| MetricsError::Overflow)?;

        match trade.side {
            TradeSide::Buy => {
                self.buys = self.buys.checked_add(1).ok_or(MetricsError::Overflow)?;
                self.buy_quote_units = self
                    .buy_quote_units
                    .checked_add(trade.quote_units)
                    .ok_or(MetricsError::Overflow)?;
            }
            TradeSide::Sell => {
                self.sells = self.sells.checked_add(1).ok_or(MetricsError::Overflow)?;
                self.sell_quote_units = self
                    .sell_quote_units
                    .checked_add(trade.quote_units)
                    .ok_or(MetricsError::Overflow)?;
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn net_quote_flow(&self) -> i128 {
        let buys = self.buy_quote_units.min(i128::MAX as u128) as i128;
        let sells = self.sell_quote_units.min(i128::MAX as u128) as i128;
        buys - sells
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QualificationRules {
    pub minimum_trades: u64,
    pub minimum_unique_traders: u64,
    pub minimum_quote_volume: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Qualification {
    Pending,
    EligibleForRiskChecks,
}

impl QualificationRules {
    #[must_use]
    pub fn evaluate(self, metrics: &RollingMarketMetrics) -> Qualification {
        let total_volume = metrics
            .buy_quote_units
            .saturating_add(metrics.sell_quote_units);

        if metrics.trades >= self.minimum_trades
            && metrics.unique_traders >= self.minimum_unique_traders
            && total_volume >= self.minimum_quote_volume
        {
            Qualification::EligibleForRiskChecks
        } else {
            Qualification::Pending
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use soldisco_domain::{MarketIdentity, Network, Venue};

    use super::{
        ObservationTarget, ObservationWindowRegistry, Qualification, QualificationRules,
        RollingMarketMetrics, TradeObservation, TradeSide,
    };

    fn pump_market(mint: &str) -> MarketIdentity {
        MarketIdentity {
            network: Network::SolanaMainnet,
            mint: mint.to_owned(),
            venue: Venue::PumpBondingCurve,
            market_address: format!("curve-{mint}"),
            quote_mint: Some("wrapped-sol".to_owned()),
        }
    }

    #[test]
    fn observation_windows_expire_without_replay_extension() {
        let windows = ObservationWindowRegistry::new(4);
        let first = windows
            .open_market(&pump_market("mint-a"), 1_000, Duration::from_millis(500))
            .expect("Pump market should be trackable");
        let replay = windows
            .open_market(&pump_market("mint-a"), 1_200, Duration::from_secs(10))
            .expect("replayed market should resolve");

        assert_eq!(replay.closes_at_unix_ms, first.closes_at_unix_ms);
        assert!(windows.is_active(&ObservationTarget::PumpMint("mint-a".to_owned()), 1_499));
        assert!(!windows.is_active(&ObservationTarget::PumpMint("mint-a".to_owned()), 1_500));
    }

    #[test]
    fn observation_window_capacity_evicts_the_earliest_close() {
        let windows = ObservationWindowRegistry::new(2);
        let _ = windows.open_market(&pump_market("mint-a"), 1_000, Duration::from_secs(1));
        let _ = windows.open_market(&pump_market("mint-b"), 1_000, Duration::from_secs(2));
        let _ = windows.open_market(&pump_market("mint-c"), 1_000, Duration::from_secs(3));

        assert!(!windows.is_active(&ObservationTarget::PumpMint("mint-a".to_owned()), 1_001));
        assert!(windows.is_active(&ObservationTarget::PumpMint("mint-b".to_owned()), 1_001));
        assert!(windows.is_active(&ObservationTarget::PumpMint("mint-c".to_owned()), 1_001));
        assert_eq!(windows.active_count(1_001), 2);
    }

    #[test]
    fn provisional_tokens_capture_early_activity_but_require_confirmation() {
        let windows = ObservationWindowRegistry::new(2);
        let provision = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_millis(500),
        );
        let matched = windows
            .matching_token(&ObservationTarget::PumpMint("mint-a".to_owned()), 1_100)
            .expect("provisional window should admit early activity");

        assert_eq!(matched, provision.token);
        assert!(!matched.is_confirmed_at(1_100));
        assert!(provision.token.confirm());
        assert!(matched.is_confirmed_at(1_100));
        assert!(!matched.is_confirmed_at(999));
        assert!(!matched.is_confirmed_at(1_500));
    }

    #[test]
    fn cancelled_provisional_tokens_invalidate_already_queued_activity() {
        let windows = ObservationWindowRegistry::new(2);
        let provision = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_millis(500),
        );
        let queued = windows
            .matching_token(&ObservationTarget::PumpMint("mint-a".to_owned()), 1_100)
            .expect("provisional activity token");

        provision.token.cancel_if_pending();

        assert!(!queued.is_open_at(1_100));
        assert!(!queued.is_confirmed_at(1_100));
        assert!(
            windows
                .matching_token(&ObservationTarget::PumpMint("mint-a".to_owned()), 1_100)
                .is_none()
        );
    }

    #[test]
    fn capacity_eviction_is_stable_when_windows_close_together() {
        let windows = ObservationWindowRegistry::new(2);
        let _ = windows.open_market(&pump_market("mint-b"), 1_000, Duration::from_secs(1));
        let _ = windows.open_market(&pump_market("mint-a"), 1_000, Duration::from_secs(1));
        let _ = windows.open_market(&pump_market("mint-c"), 1_000, Duration::from_secs(2));

        assert!(!windows.is_active(&ObservationTarget::PumpMint("mint-a".to_owned()), 1_001));
        assert!(windows.is_active(&ObservationTarget::PumpMint("mint-b".to_owned()), 1_001));
        assert!(windows.is_active(&ObservationTarget::PumpMint("mint-c".to_owned()), 1_001));
    }

    #[test]
    fn qualification_uses_real_activity_not_raydium_presence() {
        let mut metrics = RollingMarketMetrics::default();
        metrics
            .record(&TradeObservation {
                side: TradeSide::Buy,
                wallet: "wallet-a".to_owned(),
                base_units: 10,
                quote_units: 100,
            })
            .expect("metric should record");
        metrics
            .record(&TradeObservation {
                side: TradeSide::Sell,
                wallet: "wallet-b".to_owned(),
                base_units: 4,
                quote_units: 50,
            })
            .expect("metric should record");

        let result = QualificationRules {
            minimum_trades: 2,
            minimum_unique_traders: 2,
            minimum_quote_volume: 150,
        }
        .evaluate(&metrics);

        assert_eq!(result, Qualification::EligibleForRiskChecks);
    }
}
