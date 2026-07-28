use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::Duration,
};

use serde::Serialize;
pub use soldisco_domain::TradeSide;
use soldisco_domain::{MarketIdentity, SourceProgram, Venue};

mod qualification;
mod snapshot;

pub use qualification::{
    CANONICAL_USDC_MINT, NATIVE_SOL_MINT, QualificationAssessment, QualificationPolicy,
    QualificationPolicyError, QualificationRuleId, QuoteAssetClass, WRAPPED_SOL_MINT,
    classify_quote_asset, evaluate_qualification,
};
pub use snapshot::{
    BASIS_POINTS_SCALE, CreatorVolumeShare, LiquidityDirection, LiquiditySample,
    MarketWindowMetrics, MarketWindowSnapshot, PricePoint, PriceRatio, PriceSummary,
    ReserveExtremum, ReserveSample, ReserveSummary, SnapshotCompleteness, SnapshotError,
    TradeSample, WalletMetrics, WalletVolumeShare, build_market_window_snapshot,
};

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
const WINDOW_CANCELLED_PENDING: u8 = 2;
const WINDOW_CANCELLED_CONFIRMED: u8 = 3;

const WINDOW_COMPLETE: u8 = 0;
const WINDOW_CAPACITY_EVICTED: u8 = 1;
const WINDOW_QUEUE_OVERFLOW: u8 = 2;
const WINDOW_SOURCE_DISCONNECTED: u8 = 3;
const WINDOW_STREAM_STOPPED: u8 = 4;
const WINDOW_PIPELINE_RESTARTED: u8 = 5;
const WINDOW_DECODE_GAP: u8 = 6;
const WINDOW_PROCESSING_FAILED: u8 = 7;
const WINDOW_CPI_INSTRUCTION_DATA_UNAVAILABLE: u8 = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WindowIncompleteReason {
    CapacityEvicted,
    QueueOverflow,
    SourceDisconnected,
    StreamStopped,
    PipelineRestarted,
    DecodeGap,
    ProcessingFailed,
    CpiInstructionDataUnavailable,
}

impl WindowIncompleteReason {
    const fn code(self) -> u8 {
        match self {
            Self::CapacityEvicted => WINDOW_CAPACITY_EVICTED,
            Self::QueueOverflow => WINDOW_QUEUE_OVERFLOW,
            Self::SourceDisconnected => WINDOW_SOURCE_DISCONNECTED,
            Self::StreamStopped => WINDOW_STREAM_STOPPED,
            Self::PipelineRestarted => WINDOW_PIPELINE_RESTARTED,
            Self::DecodeGap => WINDOW_DECODE_GAP,
            Self::ProcessingFailed => WINDOW_PROCESSING_FAILED,
            Self::CpiInstructionDataUnavailable => WINDOW_CPI_INSTRUCTION_DATA_UNAVAILABLE,
        }
    }

    const fn from_code(code: u8) -> Option<Self> {
        match code {
            WINDOW_CAPACITY_EVICTED => Some(Self::CapacityEvicted),
            WINDOW_QUEUE_OVERFLOW => Some(Self::QueueOverflow),
            WINDOW_SOURCE_DISCONNECTED => Some(Self::SourceDisconnected),
            WINDOW_STREAM_STOPPED => Some(Self::StreamStopped),
            WINDOW_PIPELINE_RESTARTED => Some(Self::PipelineRestarted),
            WINDOW_DECODE_GAP => Some(Self::DecodeGap),
            WINDOW_PROCESSING_FAILED => Some(Self::ProcessingFailed),
            WINDOW_CPI_INSTRUCTION_DATA_UNAVAILABLE => Some(Self::CpiInstructionDataUnavailable),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CapacityEvicted => "CAPACITY_EVICTED",
            Self::QueueOverflow => "QUEUE_OVERFLOW",
            Self::SourceDisconnected => "SOURCE_DISCONNECTED",
            Self::StreamStopped => "STREAM_STOPPED",
            Self::PipelineRestarted => "PIPELINE_RESTARTED",
            Self::DecodeGap => "DECODE_GAP",
            Self::ProcessingFailed => "PROCESSING_FAILED",
            Self::CpiInstructionDataUnavailable => "CPI_INSTRUCTION_DATA_UNAVAILABLE",
        }
    }
}

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
    durable_window_id: Arc<OnceLock<i64>>,
    admitted: Arc<AtomicU64>,
    settled: Arc<AtomicU64>,
    incomplete_reason: Arc<AtomicU8>,
    finalization_claimed: Arc<AtomicBool>,
    finalized: Arc<AtomicBool>,
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
        matches!(
            self.state.load(Ordering::Acquire),
            WINDOW_PENDING | WINDOW_CONFIRMED
        ) && self.window.opened_at_unix_ms <= observed_at_unix_ms
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
            WINDOW_CANCELLED_PENDING | WINDOW_CANCELLED_CONFIRMED => {
                ObservationWindowStatus::Cancelled
            }
            _ => unreachable!("observation window has an invalid state"),
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
            WINDOW_CANCELLED_PENDING,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    /// Records the first collection failure without changing the window's
    /// receipt-time boundary. Later failures cannot obscure the first cause.
    #[must_use]
    pub fn mark_incomplete(&self, reason: WindowIncompleteReason) -> bool {
        self.incomplete_reason
            .compare_exchange(
                WINDOW_COMPLETE,
                reason.code(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    #[must_use]
    pub fn incomplete_reason(&self) -> Option<WindowIncompleteReason> {
        WindowIncompleteReason::from_code(self.incomplete_reason.load(Ordering::Acquire))
    }

    /// Closes a token and records why a complete window can no longer be
    /// claimed. A never-confirmed token stays a provisional cancellation and
    /// cannot be assigned a durable window id.
    pub fn cancel_with_reason(&self, reason: WindowIncompleteReason) {
        loop {
            match self.state.load(Ordering::Acquire) {
                WINDOW_PENDING => {
                    if self
                        .state
                        .compare_exchange(
                            WINDOW_PENDING,
                            WINDOW_CANCELLED_PENDING,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return;
                    }
                }
                WINDOW_CONFIRMED => {
                    let _ = self.mark_incomplete(reason);
                    if self
                        .state
                        .compare_exchange(
                            WINDOW_CONFIRMED,
                            WINDOW_CANCELLED_CONFIRMED,
                            Ordering::AcqRel,
                            Ordering::Acquire,
                        )
                        .is_ok()
                    {
                        return;
                    }
                }
                WINDOW_CANCELLED_PENDING | WINDOW_CANCELLED_CONFIRMED => return,
                _ => unreachable!("observation window has an invalid state"),
            }
        }
    }

    #[must_use]
    pub fn was_confirmed(&self) -> bool {
        matches!(
            self.state.load(Ordering::Acquire),
            WINDOW_CONFIRMED | WINDOW_CANCELLED_CONFIRMED
        )
    }

    /// Sets the positive database id exactly once. Repeating the same id is
    /// idempotent; a different id or a provisional token is rejected.
    #[must_use]
    pub fn set_durable_window_id(&self, durable_window_id: i64) -> bool {
        if durable_window_id <= 0 || !self.was_confirmed() {
            return false;
        }
        match self.durable_window_id.get() {
            Some(existing) => *existing == durable_window_id,
            None => match self.durable_window_id.set(durable_window_id) {
                Ok(()) => true,
                Err(_) => self.durable_window_id.get() == Some(&durable_window_id),
            },
        }
    }

    #[must_use]
    pub fn durable_window_id(&self) -> Option<i64> {
        self.durable_window_id.get().copied()
    }

    /// Records one queued observation before it can be visible to a worker.
    /// Returns false only on counter overflow or after finalization.
    #[must_use]
    pub fn mark_admitted(&self) -> bool {
        if self.is_finalized() {
            return false;
        }
        self.admitted
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |value| {
                value.checked_add(1)
            })
            .is_ok()
    }

    /// Records one terminal processing outcome. It cannot exceed the admitted
    /// count, which makes duplicate settlement observable instead of wrapping.
    #[must_use]
    pub fn mark_settled(&self) -> bool {
        loop {
            let admitted = self.admitted.load(Ordering::Acquire);
            let settled = self.settled.load(Ordering::Acquire);
            if settled >= admitted {
                return false;
            }
            if self
                .settled
                .compare_exchange_weak(settled, settled + 1, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return true;
            }
        }
    }

    #[must_use]
    pub fn admitted_count(&self) -> u64 {
        self.admitted.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn settled_count(&self) -> u64 {
        self.settled.load(Ordering::Acquire)
    }

    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.admitted_count() == self.settled_count()
    }

    fn try_claim_finalization(&self) -> bool {
        self.finalization_claimed
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn release_finalization(&self) {
        self.finalization_claimed.store(false, Ordering::Release);
    }

    fn is_finalization_claimed(&self) -> bool {
        self.finalization_claimed.load(Ordering::Acquire)
    }

    /// Marks a successfully committed finalization. The first caller wins.
    #[must_use]
    pub fn mark_finalized(&self) -> bool {
        self.finalized
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    #[must_use]
    pub fn is_finalized(&self) -> bool {
        self.finalized.load(Ordering::Acquire)
    }
}

impl PartialEq for ObservationWindowToken {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation && self.window == other.window
    }
}

impl Eq for ObservationWindowToken {}

/// An exclusive, registry-ordered attempt to durably finalize one window.
///
/// The completeness reason is frozen while holding the registry lock shared
/// with source-disconnect and stream-stop cancellation. Dropping an
/// unsuccessful claim releases it for a later retry; committing marks the
/// token finalized before releasing the claim.
#[derive(Debug)]
pub struct ObservationWindowFinalizationClaim {
    token: ObservationWindowToken,
    incomplete_reason: Option<WindowIncompleteReason>,
}

impl ObservationWindowFinalizationClaim {
    #[must_use]
    pub fn token(&self) -> &ObservationWindowToken {
        &self.token
    }

    #[must_use]
    pub const fn incomplete_reason(&self) -> Option<WindowIncompleteReason> {
        self.incomplete_reason
    }

    #[must_use]
    pub fn mark_committed(self) -> bool {
        self.token.mark_finalized()
    }
}

impl Drop for ObservationWindowFinalizationClaim {
    fn drop(&mut self) {
        self.token.release_finalization();
    }
}

#[derive(Clone, Debug)]
pub struct ObservationWindowProvision {
    pub token: ObservationWindowToken,
    pub newly_opened: bool,
}

#[derive(Default)]
struct ObservationWindowState {
    next_generation: u64,
    windows: HashMap<ObservationTarget, ObservationWindowToken>,
    retained: Vec<ObservationWindowToken>,
    latest_source_coverage_gap: HashMap<SourceProgram, SourceCoverageGap>,
    latest_source_progress: HashMap<SourceProgram, i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceCoverageGap {
    observed_at_unix_ms: i64,
    reason: WindowIncompleteReason,
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
        rotate_closed_windows(&mut state, opened_at_unix_ms);

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
            token.cancel_with_reason(WindowIncompleteReason::CapacityEvicted);
            retain_if_durable(&mut state.retained, token);
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
            durable_window_id: Arc::new(OnceLock::new()),
            admitted: Arc::new(AtomicU64::new(0)),
            settled: Arc::new(AtomicU64::new(0)),
            incomplete_reason: Arc::new(AtomicU8::new(WINDOW_COMPLETE)),
            finalization_claimed: Arc::new(AtomicBool::new(false)),
            finalized: Arc::new(AtomicBool::new(false)),
        };
        if let Some(gap) = state
            .latest_source_coverage_gap
            .get(&target_source_program(&target))
            .copied()
            .filter(|gap| {
                opened_at_unix_ms <= gap.observed_at_unix_ms
                    && gap.observed_at_unix_ms < closes_at_unix_ms
            })
        {
            let _ = token.mark_incomplete(gap.reason);
        }
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

    /// Matches and admits one received event while holding the same registry
    /// lock used to close windows for finalization. Once finalization removes
    /// a window from the active map, no later event can increment its admitted
    /// count.
    #[must_use]
    pub fn admit_matching_token(
        &self,
        target: &ObservationTarget,
        observed_at_unix_ms: i64,
    ) -> Option<ObservationWindowToken> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let token = state
            .windows
            .get(target)
            .filter(|token| token.is_open_at(observed_at_unix_ms))?
            .clone();
        if token.mark_admitted() {
            Some(token)
        } else {
            let _ = token.mark_incomplete(WindowIncompleteReason::QueueOverflow);
            None
        }
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
        rotate_closed_windows(&mut state, now_unix_ms);
        state.windows.len()
    }

    /// Advances the healthy live-feed watermark for one source. A complete
    /// window is not finalizable until its source has delivered a later
    /// notification at or beyond the half-open close boundary. This gives
    /// queued disconnect/idle detection a chance to invalidate the window
    /// instead of racing an optimistic PASS.
    pub fn record_source_progress(&self, source_program: SourceProgram, observed_at_unix_ms: i64) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .latest_source_progress
            .entry(source_program)
            .and_modify(|latest| *latest = (*latest).max(observed_at_unix_ms))
            .or_insert(observed_at_unix_ms);
    }

    /// Records a source-wide evidence gap whose target cannot be recovered
    /// safely from the PubSub notification. Every source-dependent window open
    /// at receipt time loses the ability to claim complete evidence. The latest
    /// timestamp and exact reason also cover a provisional discovery opened
    /// immediately after the gap by a concurrent worker.
    pub fn record_source_coverage_gap(
        &self,
        source_program: SourceProgram,
        observed_at_unix_ms: i64,
        reason: WindowIncompleteReason,
    ) -> usize {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .latest_source_coverage_gap
            .entry(source_program)
            .and_modify(|latest| {
                if observed_at_unix_ms > latest.observed_at_unix_ms {
                    *latest = SourceCoverageGap {
                        observed_at_unix_ms,
                        reason,
                    };
                }
            })
            .or_insert(SourceCoverageGap {
                observed_at_unix_ms,
                reason,
            });

        state
            .windows
            .values()
            .chain(state.retained.iter())
            .filter(|token| {
                target_source_program(token.target()) == source_program
                    && !token.is_finalized()
                    && token.snapshot().opened_at_unix_ms <= observed_at_unix_ms
                    && observed_at_unix_ms < token.snapshot().closes_at_unix_ms
            })
            .filter(|token| token.mark_incomplete(reason))
            .count()
    }

    /// Returns confirmed, durable, closed windows only after every admitted
    /// observation has reached a terminal outcome.
    #[must_use]
    pub fn finalization_candidates(&self, now_unix_ms: i64) -> Vec<ObservationWindowToken> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        rotate_closed_windows(&mut state, now_unix_ms);
        let mut candidates = state
            .retained
            .iter()
            .filter(|token| {
                token.was_confirmed()
                    && token.durable_window_id().is_some()
                    && token.is_settled()
                    && !token.is_finalization_claimed()
                    && !token.is_finalized()
            })
            .cloned()
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            (
                left.window.closes_at_unix_ms,
                &left.window.target,
                left.generation,
            )
                .cmp(&(
                    right.window.closes_at_unix_ms,
                    &right.window.target,
                    right.generation,
                ))
        });
        candidates
    }

    /// Claims one retained candidate for a single durable finalization
    /// attempt. Cancellation and this claim share the registry lock: a
    /// cancellation that wins is frozen into the claim, while a claim that
    /// wins is no longer mutable by a later stop or disconnect. A complete
    /// claim can win only after source progress reached the half-open close
    /// boundary and all admitted work settled.
    #[must_use]
    pub fn claim_finalization(
        &self,
        candidate: &ObservationWindowToken,
    ) -> Option<ObservationWindowFinalizationClaim> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let token = state.retained.iter().find(|token| {
            *token == candidate
                && Arc::ptr_eq(&token.state, &candidate.state)
                && token.was_confirmed()
                && token.durable_window_id().is_some()
                && token.is_settled()
                && !token.is_finalized()
        })?;
        if token.incomplete_reason().is_none()
            && state
                .latest_source_progress
                .get(&target_source_program(token.target()))
                .is_none_or(|watermark| *watermark < token.snapshot().closes_at_unix_ms)
        {
            return None;
        }
        if !token.try_claim_finalization() {
            return None;
        }
        Some(ObservationWindowFinalizationClaim {
            token: token.clone(),
            incomplete_reason: token.incomplete_reason(),
        })
    }

    /// Removes tokens only after their durable finalization committed.
    pub fn cleanup_finalized(&self) -> usize {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let before = state.retained.len();
        state.retained.retain(|token| !token.is_finalized());
        before - state.retained.len()
    }

    /// Stops every confirmed window that has not already crossed the
    /// finalization-claim boundary and retains it for incomplete finalization.
    /// Provisional tokens are cancelled and discarded.
    ///
    /// Closed windows stay in `retained` until their durable finalization
    /// commits. They must be marked here too: a stream stop can race with the
    /// timer that moves a window out of the active map.
    pub fn cancel_all_confirmed(&self, reason: WindowIncompleteReason) -> usize {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cancel_confirmed_matching(&mut state, reason, |_| true)
    }

    /// Cancels only windows whose evidence depends on the disconnected source.
    /// A Pump outage must not invalidate an otherwise healthy PumpSwap window,
    /// and vice versa.
    pub fn cancel_source_confirmed(
        &self,
        source_program: SourceProgram,
        reason: WindowIncompleteReason,
    ) -> usize {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cancel_confirmed_matching(&mut state, reason, |target| {
            target_source_program(target) == source_program
        })
    }
}

/// Cancels matching active and naturally closed generations under the same
/// registry lock used by receipt admission, closure, and finalization
/// selection. This makes a concurrent close/cancel/finalize race linearizable:
/// cancellation either freezes its reason first, or an already eligible claim
/// wins and later cancellation leaves that frozen claim unchanged.
fn cancel_confirmed_matching(
    state: &mut ObservationWindowState,
    reason: WindowIncompleteReason,
    matches_target: impl Fn(&ObservationTarget) -> bool,
) -> usize {
    let targets = state
        .windows
        .keys()
        .filter(|target| matches_target(target))
        .cloned()
        .collect::<Vec<_>>();
    let mut cancelled = 0;
    for target in targets {
        let Some(token) = state.windows.remove(&target) else {
            continue;
        };
        if token.was_confirmed() {
            token.cancel_with_reason(reason);
            retain_if_durable(&mut state.retained, token);
            cancelled += 1;
        } else {
            token.cancel_if_pending();
        }
    }

    for token in &state.retained {
        if matches_target(token.target())
            && !token.is_finalized()
            && !token.is_finalization_claimed()
            && token.status() == ObservationWindowStatus::Confirmed
        {
            token.cancel_with_reason(reason);
            cancelled += 1;
        }
    }
    cancelled
}

const fn target_source_program(target: &ObservationTarget) -> SourceProgram {
    match target {
        ObservationTarget::PumpMint(_) => SourceProgram::Pump,
        ObservationTarget::PumpSwapPool(_) => SourceProgram::PumpSwap,
    }
}

fn rotate_closed_windows(state: &mut ObservationWindowState, now_unix_ms: i64) {
    let closed_targets = state
        .windows
        .iter()
        .filter(|(_, token)| {
            !token.is_open_at(now_unix_ms) || token.window.closes_at_unix_ms <= now_unix_ms
        })
        .map(|(target, _)| target.clone())
        .collect::<Vec<_>>();
    for target in closed_targets {
        if let Some(token) = state.windows.remove(&target) {
            retain_if_durable(&mut state.retained, token);
        }
    }
}

fn retain_if_durable(retained: &mut Vec<ObservationWindowToken>, token: ObservationWindowToken) {
    if token.was_confirmed() && !token.is_finalized() {
        retained.push(token);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Barrier},
        thread,
        time::Duration,
    };

    use soldisco_domain::{MarketIdentity, Network, SourceProgram, Venue};

    use super::{
        ObservationTarget, ObservationWindowRegistry, ObservationWindowStatus,
        WindowIncompleteReason,
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
    fn durable_window_waits_for_all_admitted_work_before_finalization() {
        let windows = ObservationWindowRegistry::new(2);
        let provision = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_millis(500),
        );
        assert!(provision.token.confirm());
        assert!(provision.token.set_durable_window_id(42));
        assert!(provision.token.set_durable_window_id(42));
        assert!(!provision.token.set_durable_window_id(43));
        assert!(provision.token.mark_admitted());
        assert!(provision.token.mark_admitted());
        assert!(provision.token.mark_settled());

        assert!(windows.finalization_candidates(1_500).is_empty());
        assert!(provision.token.mark_settled());
        assert!(!provision.token.mark_settled());

        let candidates = windows.finalization_candidates(1_500);
        assert_eq!(candidates, vec![provision.token.clone()]);
        assert_eq!(candidates[0].durable_window_id(), Some(42));
        assert_eq!(candidates[0].admitted_count(), 2);
        assert_eq!(candidates[0].settled_count(), 2);
        assert!(candidates[0].mark_finalized());
        assert!(!candidates[0].mark_finalized());
        assert_eq!(windows.cleanup_finalized(), 1);
        assert!(windows.finalization_candidates(1_500).is_empty());
    }

    #[test]
    fn receipt_admission_and_window_closure_share_one_registry_boundary() {
        let target = ObservationTarget::PumpMint("mint-a".to_owned());
        let windows = ObservationWindowRegistry::new(2);
        let provision = windows.provision_target(
            target.clone(),
            "mint-a".to_owned(),
            1_000,
            Duration::from_millis(500),
        );
        assert!(provision.token.confirm());
        assert!(provision.token.set_durable_window_id(42));

        let admitted = windows
            .admit_matching_token(&target, 1_499)
            .expect("event received before close is admitted");
        assert!(windows.finalization_candidates(1_500).is_empty());
        assert!(
            windows.admit_matching_token(&target, 1_499).is_none(),
            "once closure removes the active generation, late processing cannot admit more work"
        );

        assert!(admitted.mark_settled());
        assert_eq!(
            windows.finalization_candidates(1_500),
            vec![provision.token]
        );
    }

    #[test]
    fn confirmed_capacity_eviction_is_retained_with_first_reason() {
        let windows = ObservationWindowRegistry::new(1);
        let first = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_secs(10),
        );
        assert!(first.token.confirm());
        assert!(first.token.set_durable_window_id(1));
        assert!(
            first
                .token
                .mark_incomplete(WindowIncompleteReason::QueueOverflow)
        );
        assert!(
            !first
                .token
                .mark_incomplete(WindowIncompleteReason::SourceDisconnected)
        );

        let _replacement = windows.provision_target(
            ObservationTarget::PumpMint("mint-b".to_owned()),
            "mint-b".to_owned(),
            1_100,
            Duration::from_secs(10),
        );

        assert_eq!(first.token.status(), ObservationWindowStatus::Cancelled);
        assert_eq!(
            first.token.incomplete_reason(),
            Some(WindowIncompleteReason::QueueOverflow)
        );
        assert_eq!(windows.finalization_candidates(1_100), vec![first.token]);
    }

    #[test]
    fn provisional_cancellation_cannot_become_a_durable_window() {
        let windows = ObservationWindowRegistry::new(1);
        let provisional = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_secs(1),
        );

        provisional.token.cancel_if_pending();

        assert_eq!(
            provisional.token.status(),
            ObservationWindowStatus::Cancelled
        );
        assert!(!provisional.token.was_confirmed());
        assert!(!provisional.token.set_durable_window_id(1));
        assert!(windows.finalization_candidates(2_000).is_empty());
    }

    #[test]
    fn cancelling_confirmed_windows_retains_them_as_incomplete() {
        let windows = ObservationWindowRegistry::new(2);
        let confirmed = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_secs(10),
        );
        let provisional = windows.provision_target(
            ObservationTarget::PumpMint("mint-b".to_owned()),
            "mint-b".to_owned(),
            1_000,
            Duration::from_secs(10),
        );
        assert!(confirmed.token.confirm());
        assert!(confirmed.token.set_durable_window_id(1));

        assert_eq!(
            windows.cancel_all_confirmed(WindowIncompleteReason::StreamStopped),
            1
        );
        assert_eq!(
            confirmed.token.incomplete_reason(),
            Some(WindowIncompleteReason::StreamStopped)
        );
        assert_eq!(
            provisional.token.status(),
            ObservationWindowStatus::Cancelled
        );
        assert_eq!(
            windows.finalization_candidates(1_001),
            vec![confirmed.token]
        );
    }

    #[test]
    fn source_disconnect_cancels_only_dependent_windows() {
        let windows = ObservationWindowRegistry::new(2);
        let pump = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_secs(5),
        );
        let swap = windows.provision_target(
            ObservationTarget::PumpSwapPool("pool-b".to_owned()),
            "mint-b".to_owned(),
            1_000,
            Duration::from_secs(5),
        );
        for (id, token) in [(1, &pump.token), (2, &swap.token)] {
            assert!(token.confirm());
            assert!(token.set_durable_window_id(id));
        }

        assert_eq!(
            windows.cancel_source_confirmed(
                SourceProgram::Pump,
                WindowIncompleteReason::SourceDisconnected,
            ),
            1
        );
        assert_eq!(pump.token.status(), ObservationWindowStatus::Cancelled);
        assert_eq!(swap.token.status(), ObservationWindowStatus::Confirmed);
        assert!(windows.is_active(&ObservationTarget::PumpSwapPool("pool-b".to_owned()), 1_100));
    }

    #[test]
    fn source_disconnect_marks_already_closed_retained_window_incomplete() {
        let windows = ObservationWindowRegistry::new(2);
        let pump = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_millis(500),
        );
        let swap = windows.provision_target(
            ObservationTarget::PumpSwapPool("pool-b".to_owned()),
            "mint-b".to_owned(),
            1_000,
            Duration::from_millis(500),
        );
        for (id, token) in [(1, &pump.token), (2, &swap.token)] {
            assert!(token.confirm());
            assert!(token.set_durable_window_id(id));
        }

        assert_eq!(windows.active_count(1_500), 0);
        assert_eq!(
            windows.cancel_source_confirmed(
                SourceProgram::Pump,
                WindowIncompleteReason::SourceDisconnected,
            ),
            1
        );

        assert_eq!(pump.token.status(), ObservationWindowStatus::Cancelled);
        assert_eq!(
            pump.token.incomplete_reason(),
            Some(WindowIncompleteReason::SourceDisconnected)
        );
        assert_eq!(swap.token.status(), ObservationWindowStatus::Confirmed);
        assert_eq!(swap.token.incomplete_reason(), None);
        let candidates = windows.finalization_candidates(1_500);
        assert_eq!(candidates, vec![pump.token.clone(), swap.token]);
        let claim = windows
            .claim_finalization(&candidates[0])
            .expect("closed Pump window should be claimable");
        assert_eq!(
            claim.incomplete_reason(),
            Some(WindowIncompleteReason::SourceDisconnected)
        );
    }

    #[test]
    fn cancel_all_racing_natural_close_marks_window_incomplete_before_finalization() {
        let windows = ObservationWindowRegistry::new(1);
        let provision = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_millis(500),
        );
        assert!(provision.token.confirm());
        assert!(provision.token.set_durable_window_id(1));

        let barrier = Arc::new(Barrier::new(3));
        let closing_windows = windows.clone();
        let closing_barrier = barrier.clone();
        let close = thread::spawn(move || {
            closing_barrier.wait();
            closing_windows.active_count(1_500)
        });
        let cancelling_windows = windows.clone();
        let cancelling_barrier = barrier.clone();
        let cancel = thread::spawn(move || {
            cancelling_barrier.wait();
            cancelling_windows.cancel_all_confirmed(WindowIncompleteReason::StreamStopped)
        });

        barrier.wait();
        assert_eq!(close.join().expect("close worker should not panic"), 0);
        assert_eq!(cancel.join().expect("cancel worker should not panic"), 1);
        assert_eq!(provision.token.status(), ObservationWindowStatus::Cancelled);
        assert_eq!(
            provision.token.incomplete_reason(),
            Some(WindowIncompleteReason::StreamStopped)
        );
        let candidates = windows.finalization_candidates(1_500);
        assert_eq!(candidates, vec![provision.token]);
        let claim = windows
            .claim_finalization(&candidates[0])
            .expect("closed Pump window should be claimable");
        assert_eq!(
            claim.incomplete_reason(),
            Some(WindowIncompleteReason::StreamStopped)
        );
    }

    #[test]
    fn finalization_claim_wins_over_a_later_disconnect() {
        let windows = ObservationWindowRegistry::new(1);
        let provision = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_millis(500),
        );
        assert!(provision.token.confirm());
        assert!(provision.token.set_durable_window_id(1));

        let candidate = windows
            .finalization_candidates(1_500)
            .pop()
            .expect("closed window should be ready");
        assert!(
            windows.claim_finalization(&candidate).is_none(),
            "a complete window must wait for source coverage through its close boundary"
        );
        windows.record_source_progress(SourceProgram::Pump, 1_499);
        assert!(windows.claim_finalization(&candidate).is_none());
        windows.record_source_progress(SourceProgram::Pump, 1_500);
        let first_claim = windows
            .claim_finalization(&candidate)
            .expect("first finalization attempt should claim the window");
        assert_eq!(first_claim.incomplete_reason(), None);
        assert!(windows.finalization_candidates(1_500).is_empty());

        assert_eq!(
            windows.cancel_source_confirmed(
                SourceProgram::Pump,
                WindowIncompleteReason::SourceDisconnected,
            ),
            0,
            "a claim that crossed the source-coverage boundary is immutable"
        );
        assert_eq!(provision.token.incomplete_reason(), None);
        drop(first_claim);

        let retry_candidate = windows
            .finalization_candidates(1_500)
            .pop()
            .expect("failed attempt should release the window for retry");
        let retry_claim = windows
            .claim_finalization(&retry_candidate)
            .expect("retry should reclaim the window");
        assert_eq!(retry_claim.incomplete_reason(), None);
    }

    #[test]
    fn source_coverage_gap_marks_only_windows_open_at_receipt_time() {
        let windows = ObservationWindowRegistry::new(3);
        let pump = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_secs(5),
        );
        let swap = windows.provision_target(
            ObservationTarget::PumpSwapPool("pool-b".to_owned()),
            "mint-b".to_owned(),
            1_000,
            Duration::from_secs(5),
        );

        assert_eq!(
            windows.record_source_coverage_gap(
                SourceProgram::Pump,
                1_100,
                WindowIncompleteReason::DecodeGap,
            ),
            1
        );
        assert_eq!(
            pump.token.incomplete_reason(),
            Some(WindowIncompleteReason::DecodeGap)
        );
        assert_eq!(swap.token.incomplete_reason(), None);

        let later = windows.provision_target(
            ObservationTarget::PumpMint("mint-c".to_owned()),
            "mint-c".to_owned(),
            1_050,
            Duration::from_secs(5),
        );
        assert_eq!(
            later.token.incomplete_reason(),
            Some(WindowIncompleteReason::DecodeGap),
            "a concurrently classified discovery must inherit a gap inside its window"
        );
    }

    #[test]
    fn delayed_source_coverage_gap_marks_a_closed_unfinalized_window() {
        let windows = ObservationWindowRegistry::new(1);
        let pump = windows.provision_target(
            ObservationTarget::PumpMint("mint-a".to_owned()),
            "mint-a".to_owned(),
            1_000,
            Duration::from_millis(500),
        );
        assert!(pump.token.confirm());
        assert!(pump.token.set_durable_window_id(1));

        assert_eq!(windows.active_count(1_500), 0);
        assert_eq!(
            windows.record_source_coverage_gap(
                SourceProgram::Pump,
                1_499,
                WindowIncompleteReason::CpiInstructionDataUnavailable,
            ),
            1
        );
        assert_eq!(
            pump.token.incomplete_reason(),
            Some(WindowIncompleteReason::CpiInstructionDataUnavailable)
        );
        let claim = windows
            .claim_finalization(&pump.token)
            .expect("closed window should remain finalizable");
        assert_eq!(
            claim.incomplete_reason(),
            Some(WindowIncompleteReason::CpiInstructionDataUnavailable)
        );
    }
}
