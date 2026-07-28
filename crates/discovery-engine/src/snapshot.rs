use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, HashSet},
};

use serde::Serialize;
use soldisco_domain::{
    MarketIdentity, NormalizedObservation, ObservationKey, ObservationPayload, SourceProgram,
    TradeSide,
};
use thiserror::Error;

pub const BASIS_POINTS_SCALE: u16 = 10_000;
const INVALID_TRADE_AMOUNT_REASON: &str = "INVALID_TRADE_AMOUNT";
const INVALID_TRADE_WALLET_REASON: &str = "INVALID_TRADE_WALLET";

/// Whether the collector can account for the entire requested window.
///
/// An incomplete window is valid audit evidence, but it must never be treated
/// as if zero missing events were observed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SnapshotCompleteness {
    Complete,
    Incomplete { reason_code: String },
}

impl SnapshotCompleteness {
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }

    #[must_use]
    pub fn reason_code(&self) -> Option<&str> {
        match self {
            Self::Complete => None,
            Self::Incomplete { reason_code } => Some(reason_code),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct WalletMetrics {
    pub trades: u64,
    pub buys: u64,
    pub sells: u64,
    pub buy_base_units: u128,
    pub sell_base_units: u128,
    pub buy_quote_units: u128,
    pub sell_quote_units: u128,
}

impl WalletMetrics {
    fn checked_record(
        &self,
        side: TradeSide,
        base_units: u128,
        quote_units: u128,
    ) -> Result<Self, SnapshotError> {
        let mut next = self.clone();
        next.trades = checked_add(next.trades, 1)?;
        match side {
            TradeSide::Buy => {
                next.buys = checked_add(next.buys, 1)?;
                next.buy_base_units = checked_add(next.buy_base_units, base_units)?;
                next.buy_quote_units = checked_add(next.buy_quote_units, quote_units)?;
            }
            TradeSide::Sell => {
                next.sells = checked_add(next.sells, 1)?;
                next.sell_base_units = checked_add(next.sell_base_units, base_units)?;
                next.sell_quote_units = checked_add(next.sell_quote_units, quote_units)?;
            }
        }
        Ok(next)
    }

    pub(crate) fn total_quote_units(&self) -> Result<u128, SnapshotError> {
        checked_add(self.buy_quote_units, self.sell_quote_units)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WalletVolumeShare {
    pub wallet: String,
    pub quote_volume_units: u128,
    pub share_bps: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CreatorVolumeShare {
    pub creator_wallets: Vec<String>,
    pub quote_volume_units: u128,
    pub share_bps: Option<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TradeSample {
    pub observation_key: ObservationKey,
    pub received_time_unix_ms: i64,
    pub source_event_time_unix_ms: Option<i64>,
    pub side: TradeSide,
    pub wallet: String,
    pub base_amount_units: u128,
    pub quote_amount_units: u128,
    pub base_reserve_units: Option<u128>,
    pub quote_reserve_units: Option<u128>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiquidityDirection {
    Deposit,
    Withdraw,
}

/// One exact, canonically oriented liquidity change observed in the window.
///
/// Amounts and reserves are base/quote values from the normalized market
/// identity, not PumpSwap's source-account order.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LiquiditySample {
    pub observation_key: ObservationKey,
    pub received_time_unix_ms: i64,
    pub source_event_time_unix_ms: Option<i64>,
    pub direction: LiquidityDirection,
    pub provider: String,
    pub base_amount_units: u128,
    pub quote_amount_units: u128,
    pub base_reserve_units: u128,
    pub quote_reserve_units: u128,
    pub lp_token_amount_units: u128,
    pub lp_token_supply_units: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReserveSample {
    pub observation_key: ObservationKey,
    pub received_time_unix_ms: i64,
    pub base_units: Option<u128>,
    pub quote_units: Option<u128>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReserveExtremum {
    pub observation_key: ObservationKey,
    pub received_time_unix_ms: i64,
    pub units: u128,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReserveSummary {
    pub first: ReserveSample,
    pub latest: ReserveSample,
    pub minimum_base: Option<ReserveExtremum>,
    pub maximum_base: Option<ReserveExtremum>,
    pub minimum_quote: Option<ReserveExtremum>,
    pub maximum_quote: Option<ReserveExtremum>,
    pub samples: Vec<ReserveSample>,
}

/// Exact quote/base price. The fraction is reduced to canonical terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PriceRatio {
    pub quote_units: u128,
    pub base_units: u128,
}

impl PriceRatio {
    fn new(quote_units: u128, base_units: u128) -> Result<Self, SnapshotError> {
        if quote_units == 0 || base_units == 0 {
            return Err(SnapshotError::ZeroTradeAmount);
        }
        let divisor = greatest_common_divisor(quote_units, base_units);
        Ok(Self {
            quote_units: quote_units / divisor,
            base_units: base_units / divisor,
        })
    }

    fn checked_cmp(self, other: Self) -> Result<Ordering, SnapshotError> {
        let left = self
            .quote_units
            .checked_mul(other.base_units)
            .ok_or(SnapshotError::ArithmeticOverflow)?;
        let right = other
            .quote_units
            .checked_mul(self.base_units)
            .ok_or(SnapshotError::ArithmeticOverflow)?;
        Ok(left.cmp(&right))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PricePoint {
    pub observation_key: ObservationKey,
    pub received_time_unix_ms: i64,
    pub ratio: PriceRatio,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PriceSummary {
    pub first: PricePoint,
    pub last: PricePoint,
    pub low: PricePoint,
    pub high: PricePoint,
    /// Exact signed change when it is representable. Extreme but valid ratios
    /// keep their price points and leave this non-qualification metric absent.
    pub change_bps: Option<i128>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MarketWindowMetrics {
    pub observations: u64,
    pub trades: u64,
    pub buys: u64,
    pub sells: u64,
    pub buy_base_units: u128,
    pub sell_base_units: u128,
    pub buy_quote_units: u128,
    pub sell_quote_units: u128,
    pub unique_traders: u64,
    pub unique_buyers: u64,
    pub unique_sellers: u64,
    pub wallets: BTreeMap<String, WalletMetrics>,
    pub largest_wallet_quote_volume: Option<WalletVolumeShare>,
    pub creator_quote_volume: Option<CreatorVolumeShare>,
    pub first_trade_received_at_unix_ms: Option<i64>,
    pub last_trade_received_at_unix_ms: Option<i64>,
    pub first_trade_slot: Option<u64>,
    pub last_trade_slot: Option<u64>,
}

impl MarketWindowMetrics {
    pub fn total_quote_volume_units(&self) -> Result<u128, SnapshotError> {
        checked_add(self.buy_quote_units, self.sell_quote_units)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MarketWindowSnapshot {
    pub market: MarketIdentity,
    pub opened_at_unix_ms: i64,
    pub closes_at_unix_ms: i64,
    pub completeness: SnapshotCompleteness,
    pub observation_keys: Vec<ObservationKey>,
    pub trade_samples: Vec<TradeSample>,
    pub liquidity_samples: Vec<LiquiditySample>,
    pub metrics: MarketWindowMetrics,
    pub reserves: Option<ReserveSummary>,
    pub prices: Option<PriceSummary>,
    pub completion_observed: bool,
    pub migration_observed: bool,
    pub completion_observation_keys: Vec<ObservationKey>,
    pub migration_observation_keys: Vec<ObservationKey>,
    pub latest_observed_slot: Option<u64>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SnapshotError {
    #[error("window close must be later than window open")]
    InvalidWindowBounds,
    #[error("an incomplete window must provide a non-empty reason code")]
    MissingIncompleteReason,
    #[error("duplicate observation key in the requested market window: {0:?}")]
    DuplicateObservation(ObservationKey),
    #[error("trade amounts must both be non-zero")]
    ZeroTradeAmount,
    #[error("a wallet address must not be empty")]
    EmptyWallet,
    #[error("checked metric arithmetic overflowed")]
    ArithmeticOverflow,
}

/// Builds an immutable, deterministic view of one exact market and one
/// half-open receipt-time interval: `[opened_at_unix_ms, closes_at_unix_ms)`.
///
/// Observations for other markets or outside that interval are ignored. A
/// duplicate key inside the exact slice is rejected rather than double-counted.
pub fn build_market_window_snapshot(
    market: &MarketIdentity,
    opened_at_unix_ms: i64,
    closes_at_unix_ms: i64,
    completeness: SnapshotCompleteness,
    observations: &[NormalizedObservation],
) -> Result<MarketWindowSnapshot, SnapshotError> {
    if closes_at_unix_ms <= opened_at_unix_ms {
        return Err(SnapshotError::InvalidWindowBounds);
    }
    if matches!(
        &completeness,
        SnapshotCompleteness::Incomplete { reason_code } if reason_code.trim().is_empty()
    ) {
        return Err(SnapshotError::MissingIncompleteReason);
    }

    let mut completeness = completeness;
    let mut matching = observations
        .iter()
        .filter(|observation| {
            observation.market == *market
                && opened_at_unix_ms <= observation.received_time_unix_ms
                && observation.received_time_unix_ms < closes_at_unix_ms
        })
        .collect::<Vec<_>>();
    matching.sort_by(|left, right| observation_order(left, right));

    let mut seen = HashSet::with_capacity(matching.len());
    for observation in &matching {
        if !seen.insert(observation.key.clone()) {
            return Err(SnapshotError::DuplicateObservation(observation.key.clone()));
        }
    }

    let creator_wallets = matching
        .iter()
        .filter_map(|observation| match &observation.payload {
            ObservationPayload::TokenCreated { creator, .. }
            | ObservationPayload::MarketCreated { creator, .. } => Some(creator.clone()),
            _ => None,
        })
        .filter(|creator| !creator.is_empty())
        .collect::<BTreeSet<_>>();

    let mut accumulator = MetricsAccumulator::default();
    let mut trade_samples = Vec::new();
    let mut liquidity_samples = Vec::new();
    let mut reserve_samples = Vec::new();
    let mut price_points = Vec::new();
    let mut completion_observation_keys = Vec::new();
    let mut migration_observation_keys = Vec::new();
    let mut observation_keys = Vec::with_capacity(matching.len());
    let mut latest_observed_slot = None;

    for observation in matching {
        observation_keys.push(observation.key.clone());
        latest_observed_slot = Some(
            latest_observed_slot.map_or(observation.key.coordinate.slot, |slot: u64| {
                slot.max(observation.key.coordinate.slot)
            }),
        );
        match &observation.payload {
            ObservationPayload::Trade {
                side,
                wallet,
                base_amount_units,
                quote_amount_units,
                base_reserve_units,
                quote_reserve_units,
            } => {
                if wallet.trim().is_empty() {
                    mark_incomplete_if_complete(&mut completeness, INVALID_TRADE_WALLET_REASON);
                    continue;
                }
                let base_amount_units = u128::from(*base_amount_units);
                let quote_amount_units = u128::from(*quote_amount_units);
                if base_amount_units == 0 || quote_amount_units == 0 {
                    mark_incomplete_if_complete(&mut completeness, INVALID_TRADE_AMOUNT_REASON);
                    continue;
                }
                let ratio = PriceRatio::new(quote_amount_units, base_amount_units)?;
                accumulator.checked_record(*side, wallet, base_amount_units, quote_amount_units)?;

                let sample = TradeSample {
                    observation_key: observation.key.clone(),
                    received_time_unix_ms: observation.received_time_unix_ms,
                    source_event_time_unix_ms: observation.source_event_time_unix_ms,
                    side: *side,
                    wallet: wallet.clone(),
                    base_amount_units,
                    quote_amount_units,
                    base_reserve_units: base_reserve_units.map(u128::from),
                    quote_reserve_units: quote_reserve_units.map(u128::from),
                };
                if sample.base_reserve_units.is_some() || sample.quote_reserve_units.is_some() {
                    reserve_samples.push(ReserveSample {
                        observation_key: observation.key.clone(),
                        received_time_unix_ms: observation.received_time_unix_ms,
                        base_units: sample.base_reserve_units,
                        quote_units: sample.quote_reserve_units,
                    });
                }
                price_points.push(PricePoint {
                    observation_key: observation.key.clone(),
                    received_time_unix_ms: observation.received_time_unix_ms,
                    ratio,
                });
                trade_samples.push(sample);
            }
            ObservationPayload::MarketCompleted { .. } => {
                completion_observation_keys.push(observation.key.clone());
            }
            ObservationPayload::MarketMigrated { .. } => {
                migration_observation_keys.push(observation.key.clone());
            }
            ObservationPayload::LiquidityDeposited {
                provider,
                base_amount_units,
                quote_amount_units,
                base_reserve_units,
                quote_reserve_units,
                lp_token_amount_units,
                lp_token_supply_units,
                ..
            } => {
                liquidity_samples.push(LiquiditySample {
                    observation_key: observation.key.clone(),
                    received_time_unix_ms: observation.received_time_unix_ms,
                    source_event_time_unix_ms: observation.source_event_time_unix_ms,
                    direction: LiquidityDirection::Deposit,
                    provider: provider.clone(),
                    base_amount_units: u128::from(*base_amount_units),
                    quote_amount_units: u128::from(*quote_amount_units),
                    base_reserve_units: u128::from(*base_reserve_units),
                    quote_reserve_units: u128::from(*quote_reserve_units),
                    lp_token_amount_units: u128::from(*lp_token_amount_units),
                    lp_token_supply_units: u128::from(*lp_token_supply_units),
                });
                reserve_samples.push(ReserveSample {
                    observation_key: observation.key.clone(),
                    received_time_unix_ms: observation.received_time_unix_ms,
                    base_units: Some(u128::from(*base_reserve_units)),
                    quote_units: Some(u128::from(*quote_reserve_units)),
                });
            }
            ObservationPayload::LiquidityWithdrawn {
                provider,
                base_amount_units,
                quote_amount_units,
                base_reserve_units,
                quote_reserve_units,
                lp_token_amount_units,
                lp_token_supply_units,
            } => {
                liquidity_samples.push(LiquiditySample {
                    observation_key: observation.key.clone(),
                    received_time_unix_ms: observation.received_time_unix_ms,
                    source_event_time_unix_ms: observation.source_event_time_unix_ms,
                    direction: LiquidityDirection::Withdraw,
                    provider: provider.clone(),
                    base_amount_units: u128::from(*base_amount_units),
                    quote_amount_units: u128::from(*quote_amount_units),
                    base_reserve_units: u128::from(*base_reserve_units),
                    quote_reserve_units: u128::from(*quote_reserve_units),
                    lp_token_amount_units: u128::from(*lp_token_amount_units),
                    lp_token_supply_units: u128::from(*lp_token_supply_units),
                });
                reserve_samples.push(ReserveSample {
                    observation_key: observation.key.clone(),
                    received_time_unix_ms: observation.received_time_unix_ms,
                    base_units: Some(u128::from(*base_reserve_units)),
                    quote_units: Some(u128::from(*quote_reserve_units)),
                });
            }
            ObservationPayload::TokenCreated { .. } | ObservationPayload::MarketCreated { .. } => {}
        }
    }

    let observations =
        u64::try_from(observation_keys.len()).map_err(|_| SnapshotError::ArithmeticOverflow)?;
    let metrics = accumulator.finish(
        observations,
        &creator_wallets,
        trade_samples.first(),
        trade_samples.last(),
    )?;
    let reserves = summarize_reserves(reserve_samples);
    let prices = summarize_prices(price_points)?;

    Ok(MarketWindowSnapshot {
        market: market.clone(),
        opened_at_unix_ms,
        closes_at_unix_ms,
        completeness,
        observation_keys,
        trade_samples,
        liquidity_samples,
        metrics,
        reserves,
        prices,
        completion_observed: !completion_observation_keys.is_empty(),
        migration_observed: !migration_observation_keys.is_empty(),
        completion_observation_keys,
        migration_observation_keys,
        latest_observed_slot,
    })
}

fn mark_incomplete_if_complete(completeness: &mut SnapshotCompleteness, reason_code: &'static str) {
    if completeness.is_complete() {
        *completeness = SnapshotCompleteness::Incomplete {
            reason_code: reason_code.to_owned(),
        };
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct MetricsAccumulator {
    trades: u64,
    buys: u64,
    sells: u64,
    buy_base_units: u128,
    sell_base_units: u128,
    buy_quote_units: u128,
    sell_quote_units: u128,
    wallets: BTreeMap<String, WalletMetrics>,
}

impl MetricsAccumulator {
    fn checked_record(
        &mut self,
        side: TradeSide,
        wallet: &str,
        base_units: u128,
        quote_units: u128,
    ) -> Result<(), SnapshotError> {
        let next_trades = checked_add(self.trades, 1)?;
        let current_wallet = self.wallets.get(wallet).cloned().unwrap_or_default();
        let next_wallet = current_wallet.checked_record(side, base_units, quote_units)?;

        let (next_buys, next_sells, next_buy_base, next_sell_base, next_buy_quote, next_sell_quote) =
            match side {
                TradeSide::Buy => (
                    checked_add(self.buys, 1)?,
                    self.sells,
                    checked_add(self.buy_base_units, base_units)?,
                    self.sell_base_units,
                    checked_add(self.buy_quote_units, quote_units)?,
                    self.sell_quote_units,
                ),
                TradeSide::Sell => (
                    self.buys,
                    checked_add(self.sells, 1)?,
                    self.buy_base_units,
                    checked_add(self.sell_base_units, base_units)?,
                    self.buy_quote_units,
                    checked_add(self.sell_quote_units, quote_units)?,
                ),
            };

        self.trades = next_trades;
        self.buys = next_buys;
        self.sells = next_sells;
        self.buy_base_units = next_buy_base;
        self.sell_base_units = next_sell_base;
        self.buy_quote_units = next_buy_quote;
        self.sell_quote_units = next_sell_quote;
        self.wallets.insert(wallet.to_owned(), next_wallet);
        Ok(())
    }

    fn finish(
        self,
        observations: u64,
        creator_wallets: &BTreeSet<String>,
        first_trade: Option<&TradeSample>,
        last_trade: Option<&TradeSample>,
    ) -> Result<MarketWindowMetrics, SnapshotError> {
        let total_quote_volume = checked_add(self.buy_quote_units, self.sell_quote_units)?;
        let unique_traders =
            u64::try_from(self.wallets.len()).map_err(|_| SnapshotError::ArithmeticOverflow)?;
        let unique_buyers = u64::try_from(
            self.wallets
                .values()
                .filter(|metrics| metrics.buys > 0)
                .count(),
        )
        .map_err(|_| SnapshotError::ArithmeticOverflow)?;
        let unique_sellers = u64::try_from(
            self.wallets
                .values()
                .filter(|metrics| metrics.sells > 0)
                .count(),
        )
        .map_err(|_| SnapshotError::ArithmeticOverflow)?;

        let mut largest_wallet_quote_volume = None;
        for (wallet, metrics) in &self.wallets {
            let quote_volume_units = metrics.total_quote_units()?;
            let replace =
                largest_wallet_quote_volume
                    .as_ref()
                    .is_none_or(|largest: &WalletVolumeShare| {
                        quote_volume_units > largest.quote_volume_units
                    });
            if replace {
                largest_wallet_quote_volume = Some(WalletVolumeShare {
                    wallet: wallet.clone(),
                    quote_volume_units,
                    share_bps: fraction_to_basis_points(quote_volume_units, total_quote_volume)?,
                });
            }
        }

        let creator_quote_volume = if creator_wallets.is_empty() {
            None
        } else {
            let mut quote_volume_units = 0_u128;
            for creator in creator_wallets {
                if let Some(metrics) = self.wallets.get(creator) {
                    quote_volume_units =
                        checked_add(quote_volume_units, metrics.total_quote_units()?)?;
                }
            }
            Some(CreatorVolumeShare {
                creator_wallets: creator_wallets.iter().cloned().collect(),
                quote_volume_units,
                share_bps: (total_quote_volume > 0)
                    .then(|| fraction_to_basis_points(quote_volume_units, total_quote_volume))
                    .transpose()?,
            })
        };

        Ok(MarketWindowMetrics {
            observations,
            trades: self.trades,
            buys: self.buys,
            sells: self.sells,
            buy_base_units: self.buy_base_units,
            sell_base_units: self.sell_base_units,
            buy_quote_units: self.buy_quote_units,
            sell_quote_units: self.sell_quote_units,
            unique_traders,
            unique_buyers,
            unique_sellers,
            wallets: self.wallets,
            largest_wallet_quote_volume,
            creator_quote_volume,
            first_trade_received_at_unix_ms: first_trade.map(|sample| sample.received_time_unix_ms),
            last_trade_received_at_unix_ms: last_trade.map(|sample| sample.received_time_unix_ms),
            first_trade_slot: first_trade.map(|sample| sample.observation_key.coordinate.slot),
            last_trade_slot: last_trade.map(|sample| sample.observation_key.coordinate.slot),
        })
    }
}

fn summarize_reserves(samples: Vec<ReserveSample>) -> Option<ReserveSummary> {
    let first = samples.first()?.clone();
    let latest = samples.last()?.clone();
    let mut minimum_base = None;
    let mut maximum_base = None;
    let mut minimum_quote = None;
    let mut maximum_quote = None;

    for sample in &samples {
        update_extrema(
            &mut minimum_base,
            &mut maximum_base,
            sample,
            sample.base_units,
        );
        update_extrema(
            &mut minimum_quote,
            &mut maximum_quote,
            sample,
            sample.quote_units,
        );
    }

    Some(ReserveSummary {
        first,
        latest,
        minimum_base,
        maximum_base,
        minimum_quote,
        maximum_quote,
        samples,
    })
}

fn update_extrema(
    minimum: &mut Option<ReserveExtremum>,
    maximum: &mut Option<ReserveExtremum>,
    sample: &ReserveSample,
    units: Option<u128>,
) {
    let Some(units) = units else {
        return;
    };
    let candidate = || ReserveExtremum {
        observation_key: sample.observation_key.clone(),
        received_time_unix_ms: sample.received_time_unix_ms,
        units,
    };
    if minimum.as_ref().is_none_or(|current| units < current.units) {
        *minimum = Some(candidate());
    }
    if maximum.as_ref().is_none_or(|current| units > current.units) {
        *maximum = Some(candidate());
    }
}

fn summarize_prices(points: Vec<PricePoint>) -> Result<Option<PriceSummary>, SnapshotError> {
    let Some(first) = points.first().cloned() else {
        return Ok(None);
    };
    let last = points
        .last()
        .cloned()
        .expect("a first price point guarantees a last price point");
    let mut low = first.clone();
    let mut high = first.clone();
    for point in points.iter().skip(1) {
        if point.ratio.checked_cmp(low.ratio)? == Ordering::Less {
            low = point.clone();
        }
        if point.ratio.checked_cmp(high.ratio)? == Ordering::Greater {
            high = point.clone();
        }
    }

    Ok(Some(PriceSummary {
        change_bps: price_change_basis_points(first.ratio, last.ratio).ok(),
        first,
        last,
        low,
        high,
    }))
}

fn price_change_basis_points(first: PriceRatio, last: PriceRatio) -> Result<i128, SnapshotError> {
    let last_cross = last
        .quote_units
        .checked_mul(first.base_units)
        .ok_or(SnapshotError::ArithmeticOverflow)?;
    let first_cross = first
        .quote_units
        .checked_mul(last.base_units)
        .ok_or(SnapshotError::ArithmeticOverflow)?;
    let (negative, difference) = if last_cross >= first_cross {
        (false, last_cross - first_cross)
    } else {
        (true, first_cross - last_cross)
    };
    let magnitude = checked_scaled_ratio(difference, first_cross, u128::from(BASIS_POINTS_SCALE))?;
    signed_magnitude(magnitude, negative)
}

fn fraction_to_basis_points(numerator: u128, denominator: u128) -> Result<u16, SnapshotError> {
    if denominator == 0 || numerator > denominator {
        return Err(SnapshotError::ArithmeticOverflow);
    }
    let scale = u128::from(BASIS_POINTS_SCALE);
    let floor = checked_scaled_ratio(numerator, denominator, scale)?;
    let value = if fraction_within_basis_point_limit(
        numerator,
        denominator,
        u16::try_from(floor).map_err(|_| SnapshotError::ArithmeticOverflow)?,
    )? {
        floor
    } else {
        floor
            .checked_add(1)
            .ok_or(SnapshotError::ArithmeticOverflow)?
    };
    u16::try_from(value).map_err(|_| SnapshotError::ArithmeticOverflow)
}

fn fraction_within_basis_point_limit(
    numerator: u128,
    denominator: u128,
    maximum_bps: u16,
) -> Result<bool, SnapshotError> {
    if denominator == 0 || maximum_bps > BASIS_POINTS_SCALE {
        return Err(SnapshotError::ArithmeticOverflow);
    }
    let maximum_numerator = checked_scaled_ratio(
        denominator,
        u128::from(BASIS_POINTS_SCALE),
        u128::from(maximum_bps),
    )?;
    Ok(numerator <= maximum_numerator)
}

/// Computes `floor(numerator * scale / denominator)` without overflowing the
/// intermediate product.
fn checked_scaled_ratio(
    numerator: u128,
    denominator: u128,
    scale: u128,
) -> Result<u128, SnapshotError> {
    if denominator == 0 {
        return Err(SnapshotError::ArithmeticOverflow);
    }
    let whole = numerator / denominator;
    let remainder = numerator % denominator;
    let whole_scaled = whole
        .checked_mul(scale)
        .ok_or(SnapshotError::ArithmeticOverflow)?;
    let fractional = scaled_proper_fraction(remainder, denominator, scale)?;
    whole_scaled
        .checked_add(fractional)
        .ok_or(SnapshotError::ArithmeticOverflow)
}

/// Computes `floor(numerator * scale / denominator)` for
/// `numerator < denominator`, without multiplying numerator by scale.
fn scaled_proper_fraction(
    numerator: u128,
    denominator: u128,
    scale: u128,
) -> Result<u128, SnapshotError> {
    debug_assert!(numerator < denominator);
    if numerator == 0 || scale == 0 {
        return Ok(0);
    }

    let mut low = 0_u128;
    let mut high = scale;
    while low < high {
        let midpoint = low + (high - low).div_ceil(2);
        if ceil_scaled_denominator(midpoint, denominator, scale)? <= numerator {
            low = midpoint;
        } else {
            high = midpoint - 1;
        }
    }
    Ok(low)
}

/// Computes `ceil(multiplier * value / divisor)` without overflowing, under
/// the precondition `multiplier <= divisor`.
fn ceil_scaled_denominator(
    multiplier: u128,
    value: u128,
    divisor: u128,
) -> Result<u128, SnapshotError> {
    if divisor == 0 || multiplier > divisor {
        return Err(SnapshotError::ArithmeticOverflow);
    }
    let quotient = value / divisor;
    let remainder = value % divisor;
    let whole = quotient
        .checked_mul(multiplier)
        .ok_or(SnapshotError::ArithmeticOverflow)?;
    let remainder_product = remainder
        .checked_mul(multiplier)
        .ok_or(SnapshotError::ArithmeticOverflow)?;
    let rounded_fraction =
        (remainder_product / divisor) + u128::from(remainder_product % divisor != 0);
    whole
        .checked_add(rounded_fraction)
        .ok_or(SnapshotError::ArithmeticOverflow)
}

fn signed_magnitude(magnitude: u128, negative: bool) -> Result<i128, SnapshotError> {
    if negative && magnitude == (i128::MAX as u128) + 1 {
        return Ok(i128::MIN);
    }
    let magnitude = i128::try_from(magnitude).map_err(|_| SnapshotError::ArithmeticOverflow)?;
    Ok(if negative { -magnitude } else { magnitude })
}

fn greatest_common_divisor(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn checked_add<T>(left: T, right: T) -> Result<T, SnapshotError>
where
    T: CheckedAdd,
{
    left.checked_add(right)
        .ok_or(SnapshotError::ArithmeticOverflow)
}

trait CheckedAdd: Sized {
    fn checked_add(self, other: Self) -> Option<Self>;
}

impl CheckedAdd for u64 {
    fn checked_add(self, other: Self) -> Option<Self> {
        u64::checked_add(self, other)
    }
}

impl CheckedAdd for u128 {
    fn checked_add(self, other: Self) -> Option<Self> {
        u128::checked_add(self, other)
    }
}

fn observation_order(left: &NormalizedObservation, right: &NormalizedObservation) -> Ordering {
    left.received_time_unix_ms
        .cmp(&right.received_time_unix_ms)
        .then_with(|| left.key.coordinate.slot.cmp(&right.key.coordinate.slot))
        .then_with(|| {
            left.key
                .coordinate
                .transaction_index
                .cmp(&right.key.coordinate.transaction_index)
        })
        .then_with(|| {
            left.key
                .coordinate
                .signature
                .cmp(&right.key.coordinate.signature)
        })
        .then_with(|| {
            left.key
                .coordinate
                .instruction_index
                .cmp(&right.key.coordinate.instruction_index)
        })
        .then_with(|| {
            left.key
                .coordinate
                .event_index
                .cmp(&right.key.coordinate.event_index)
        })
        .then_with(|| {
            source_program_rank(left.key.program).cmp(&source_program_rank(right.key.program))
        })
}

const fn source_program_rank(program: SourceProgram) -> u8 {
    match program {
        SourceProgram::Pump => 0,
        SourceProgram::PumpSwap => 1,
        SourceProgram::RaydiumCpmm => 2,
        SourceProgram::RaydiumClmm => 3,
        SourceProgram::RaydiumAmmV4 => 4,
    }
}

#[cfg(test)]
mod tests {
    use soldisco_domain::{
        ChainCoordinate, Commitment, Network, ObservationPayload, SourceProgram, Venue,
    };

    use super::*;

    fn market() -> MarketIdentity {
        MarketIdentity {
            network: Network::SolanaMainnet,
            mint: "mint".to_owned(),
            venue: Venue::PumpBondingCurve,
            market_address: "curve".to_owned(),
            quote_mint: Some(crate::WRAPPED_SOL_MINT.to_owned()),
        }
    }

    fn observation(
        event_index: u16,
        received_time_unix_ms: i64,
        payload: ObservationPayload,
    ) -> NormalizedObservation {
        NormalizedObservation {
            key: ObservationKey {
                network: Network::SolanaMainnet,
                program: SourceProgram::Pump,
                coordinate: ChainCoordinate {
                    slot: 100 + u64::from(event_index),
                    transaction_index: Some(0),
                    signature: format!("signature-{event_index}"),
                    instruction_index: 1,
                    event_index,
                },
            },
            commitment: Commitment::Confirmed,
            schema_version: 1,
            decoder_version: "test".to_owned(),
            event_kind: "test".to_owned(),
            market: market(),
            source_event_time_unix_ms: Some(received_time_unix_ms - 5),
            received_time_unix_ms,
            raw_evidence_hash: format!("hash-{event_index}"),
            source_evidence_base64: String::new(),
            source_details: Default::default(),
            payload,
        }
    }

    fn trade(
        event_index: u16,
        time: i64,
        side: TradeSide,
        wallet: &str,
        base: u64,
        quote: u64,
        reserves: (Option<u64>, Option<u64>),
    ) -> NormalizedObservation {
        observation(
            event_index,
            time,
            ObservationPayload::Trade {
                side,
                wallet: wallet.to_owned(),
                base_amount_units: base,
                quote_amount_units: quote,
                base_reserve_units: reserves.0,
                quote_reserve_units: reserves.1,
            },
        )
    }

    #[test]
    fn snapshot_is_half_open_exact_market_and_deterministically_sorted() {
        let mut outside_market = trade(9, 150, TradeSide::Buy, "ignored", 1, 1, (None, None));
        outside_market.market.market_address = "other".to_owned();
        let observations = vec![
            trade(
                2,
                199,
                TradeSide::Sell,
                "wallet-b",
                10,
                30,
                (Some(80), Some(240)),
            ),
            trade(0, 99, TradeSide::Buy, "before", 1, 1, (None, None)),
            trade(3, 200, TradeSide::Buy, "at-close", 1, 1, (None, None)),
            trade(
                1,
                100,
                TradeSide::Buy,
                "wallet-a",
                10,
                10,
                (Some(100), Some(100)),
            ),
            outside_market,
        ];

        let snapshot = build_market_window_snapshot(
            &market(),
            100,
            200,
            SnapshotCompleteness::Complete,
            &observations,
        )
        .expect("snapshot");

        assert_eq!(snapshot.metrics.observations, 2);
        assert_eq!(snapshot.metrics.trades, 2);
        assert_eq!(snapshot.trade_samples[0].wallet, "wallet-a");
        assert_eq!(snapshot.trade_samples[1].wallet, "wallet-b");
        assert_eq!(snapshot.metrics.first_trade_received_at_unix_ms, Some(100));
        assert_eq!(snapshot.metrics.last_trade_received_at_unix_ms, Some(199));
    }

    #[test]
    fn malformed_trade_is_retained_but_cannot_enter_metrics_or_pass_as_complete() {
        let observations = vec![
            trade(1, 100, TradeSide::Buy, "valid", 10, 20, (None, None)),
            trade(2, 110, TradeSide::Buy, "dust", 1, 0, (None, None)),
        ];

        let snapshot = build_market_window_snapshot(
            &market(),
            100,
            200,
            SnapshotCompleteness::Complete,
            &observations,
        )
        .expect("invalid source evidence should degrade instead of wedging finalization");

        assert_eq!(snapshot.metrics.observations, 2);
        assert_eq!(snapshot.observation_keys.len(), 2);
        assert_eq!(snapshot.metrics.trades, 1);
        assert_eq!(snapshot.trade_samples.len(), 1);
        assert_eq!(
            snapshot.completeness,
            SnapshotCompleteness::Incomplete {
                reason_code: INVALID_TRADE_AMOUNT_REASON.to_owned(),
            }
        );
    }

    #[test]
    fn counts_duplicate_wallets_and_creator_concentration_exactly() {
        let observations = vec![
            observation(
                0,
                100,
                ObservationPayload::TokenCreated {
                    name: "Token".to_owned(),
                    symbol: "TOK".to_owned(),
                    uri: "uri".to_owned(),
                    creator: "wallet-a".to_owned(),
                    user: "wallet-a".to_owned(),
                },
            ),
            trade(1, 110, TradeSide::Buy, "wallet-a", 10, 60, (None, None)),
            trade(2, 120, TradeSide::Sell, "wallet-a", 5, 20, (None, None)),
            trade(3, 130, TradeSide::Buy, "wallet-b", 10, 20, (None, None)),
        ];

        let snapshot = build_market_window_snapshot(
            &market(),
            100,
            200,
            SnapshotCompleteness::Complete,
            &observations,
        )
        .expect("snapshot");

        assert_eq!(snapshot.metrics.unique_traders, 2);
        assert_eq!(snapshot.metrics.unique_buyers, 2);
        assert_eq!(snapshot.metrics.unique_sellers, 1);
        assert_eq!(snapshot.metrics.wallets["wallet-a"].trades, 2);
        assert_eq!(
            snapshot.metrics.largest_wallet_quote_volume,
            Some(WalletVolumeShare {
                wallet: "wallet-a".to_owned(),
                quote_volume_units: 80,
                share_bps: 8_000,
            })
        );
        assert_eq!(
            snapshot.metrics.creator_quote_volume,
            Some(CreatorVolumeShare {
                creator_wallets: vec!["wallet-a".to_owned()],
                quote_volume_units: 80,
                share_bps: Some(8_000),
            })
        );
    }

    #[test]
    fn captures_reserve_extrema_prices_and_signed_change_without_floats() {
        let observations = vec![
            trade(1, 100, TradeSide::Buy, "a", 3, 6, (Some(100), Some(200))),
            trade(2, 110, TradeSide::Buy, "b", 2, 8, (Some(80), Some(320))),
            trade(3, 120, TradeSide::Sell, "c", 10, 10, (Some(120), Some(120))),
        ];
        let snapshot = build_market_window_snapshot(
            &market(),
            100,
            200,
            SnapshotCompleteness::Complete,
            &observations,
        )
        .expect("snapshot");
        let prices = snapshot.prices.expect("prices");
        let reserves = snapshot.reserves.expect("reserves");

        assert_eq!(
            prices.first.ratio,
            PriceRatio {
                quote_units: 2,
                base_units: 1,
            }
        );
        assert_eq!(
            prices.high.ratio,
            PriceRatio {
                quote_units: 4,
                base_units: 1,
            }
        );
        assert_eq!(
            prices.low.ratio,
            PriceRatio {
                quote_units: 1,
                base_units: 1,
            }
        );
        assert_eq!(prices.change_bps, Some(-5_000));
        assert_eq!(reserves.minimum_base.expect("min base").units, 80);
        assert_eq!(reserves.maximum_quote.expect("max quote").units, 320);
        assert_eq!(reserves.first.received_time_unix_ms, 100);
        assert_eq!(reserves.latest.received_time_unix_ms, 120);
    }

    #[test]
    fn liquidity_changes_contribute_canonical_reserve_samples_without_trade_volume() {
        let observations = vec![
            observation(
                1,
                100,
                ObservationPayload::LiquidityDeposited {
                    provider: "provider-a".to_owned(),
                    base_amount_units: 20,
                    quote_amount_units: 40,
                    base_reserve_units: 80,
                    quote_reserve_units: 160,
                    lp_token_amount_units: 10,
                    lp_token_supply_units: 1_000,
                },
            ),
            trade(
                2,
                110,
                TradeSide::Buy,
                "trader",
                5,
                10,
                (Some(100), Some(200)),
            ),
            observation(
                3,
                120,
                ObservationPayload::LiquidityWithdrawn {
                    provider: "provider-b".to_owned(),
                    base_amount_units: 10,
                    quote_amount_units: 20,
                    base_reserve_units: 70,
                    quote_reserve_units: 140,
                    lp_token_amount_units: 5,
                    lp_token_supply_units: 995,
                },
            ),
        ];
        let snapshot = build_market_window_snapshot(
            &market(),
            100,
            200,
            SnapshotCompleteness::Complete,
            &observations,
        )
        .expect("liquidity observations should produce a complete snapshot");
        let reserves = snapshot.reserves.expect("reserve summary");

        assert_eq!(snapshot.metrics.observations, 3);
        assert_eq!(snapshot.metrics.trades, 1);
        assert_eq!(snapshot.metrics.buy_base_units, 5);
        assert_eq!(snapshot.metrics.buy_quote_units, 10);
        assert_eq!(snapshot.liquidity_samples.len(), 2);
        assert_eq!(
            snapshot.liquidity_samples[0],
            LiquiditySample {
                observation_key: observations[0].key.clone(),
                received_time_unix_ms: 100,
                source_event_time_unix_ms: Some(95),
                direction: LiquidityDirection::Deposit,
                provider: "provider-a".to_owned(),
                base_amount_units: 20,
                quote_amount_units: 40,
                base_reserve_units: 80,
                quote_reserve_units: 160,
                lp_token_amount_units: 10,
                lp_token_supply_units: 1_000,
            }
        );
        assert_eq!(
            snapshot.liquidity_samples[1],
            LiquiditySample {
                observation_key: observations[2].key.clone(),
                received_time_unix_ms: 120,
                source_event_time_unix_ms: Some(115),
                direction: LiquidityDirection::Withdraw,
                provider: "provider-b".to_owned(),
                base_amount_units: 10,
                quote_amount_units: 20,
                base_reserve_units: 70,
                quote_reserve_units: 140,
                lp_token_amount_units: 5,
                lp_token_supply_units: 995,
            }
        );
        assert_eq!(reserves.samples.len(), 3);
        assert_eq!(reserves.first.base_units, Some(80));
        assert_eq!(reserves.latest.base_units, Some(70));
        assert_eq!(reserves.minimum_base.expect("minimum base").units, 70);
        assert_eq!(reserves.maximum_quote.expect("maximum quote").units, 200);
        assert!(snapshot.completeness.is_complete());
    }

    #[test]
    fn records_completion_and_migration_flags_and_keys() {
        let observations = vec![
            observation(
                1,
                110,
                ObservationPayload::MarketCompleted {
                    user: "user".to_owned(),
                },
            ),
            observation(
                2,
                120,
                ObservationPayload::MarketMigrated {
                    user: "user".to_owned(),
                    destination_market: "raydium".to_owned(),
                    base_amount_units: 1,
                    legacy_sol_amount_units: 2,
                },
            ),
        ];
        let snapshot = build_market_window_snapshot(
            &market(),
            100,
            200,
            SnapshotCompleteness::Complete,
            &observations,
        )
        .expect("snapshot");

        assert!(snapshot.completion_observed);
        assert!(snapshot.migration_observed);
        assert_eq!(snapshot.completion_observation_keys.len(), 1);
        assert_eq!(snapshot.migration_observation_keys.len(), 1);
    }

    #[test]
    fn rejects_duplicate_observation_keys() {
        let item = trade(1, 100, TradeSide::Buy, "a", 1, 1, (None, None));
        let error = build_market_window_snapshot(
            &market(),
            100,
            200,
            SnapshotCompleteness::Complete,
            &[item.clone(), item],
        )
        .expect_err("duplicate should fail");

        assert!(matches!(error, SnapshotError::DuplicateObservation(_)));
    }

    #[test]
    fn durable_snapshot_json_preserves_aggregates_above_u64() {
        let snapshot = build_market_window_snapshot(
            &market(),
            100,
            200,
            SnapshotCompleteness::Complete,
            &[
                trade(
                    1,
                    100,
                    TradeSide::Buy,
                    "wallet-a",
                    u64::MAX,
                    u64::MAX,
                    (None, None),
                ),
                trade(
                    2,
                    101,
                    TradeSide::Buy,
                    "wallet-a",
                    u64::MAX,
                    u64::MAX,
                    (None, None),
                ),
            ],
        )
        .expect("snapshot");

        assert!(snapshot.metrics.total_quote_volume_units().expect("sum") > u128::from(u64::MAX));
        let encoded = serde_json::to_value(&snapshot)
            .expect("arbitrary-precision JSON must preserve on-chain aggregates");
        assert_eq!(
            encoded["metrics"]["buy_quote_units"].to_string(),
            (u128::from(u64::MAX) * 2).to_string()
        );
    }

    #[test]
    fn checked_accumulator_does_not_partially_mutate_on_overflow() {
        let original = MetricsAccumulator {
            trades: u64::MAX,
            ..MetricsAccumulator::default()
        };
        let mut accumulator = original.clone();
        let error = accumulator
            .checked_record(TradeSide::Buy, "wallet", 1, 1)
            .expect_err("overflow");

        assert_eq!(error, SnapshotError::ArithmeticOverflow);
        assert_eq!(accumulator, original);
    }

    #[test]
    fn checked_ratio_handles_values_that_would_overflow_direct_multiplication() {
        assert_eq!(
            checked_scaled_ratio(u128::MAX - 1, u128::MAX, 10_000).expect("ratio"),
            9_999
        );
        assert_eq!(
            fraction_to_basis_points(u128::MAX, u128::MAX).expect("share"),
            10_000
        );
        assert_eq!(
            fraction_to_basis_points(9_001, 10_001).expect("conservative share"),
            9_001
        );
    }

    #[test]
    fn price_change_reports_unrepresentable_result_as_overflow() {
        let first = PriceRatio {
            quote_units: 1,
            base_units: u128::MAX,
        };
        let last = PriceRatio {
            quote_units: u128::MAX,
            base_units: 1,
        };

        assert_eq!(
            price_change_basis_points(first, last),
            Err(SnapshotError::ArithmeticOverflow)
        );
    }

    #[test]
    fn unrepresentable_price_change_does_not_block_snapshot_finalization() {
        let snapshot = build_market_window_snapshot(
            &market(),
            100,
            200,
            SnapshotCompleteness::Complete,
            &[
                trade(
                    1,
                    100,
                    TradeSide::Buy,
                    "wallet-a",
                    u64::MAX,
                    1,
                    (None, None),
                ),
                trade(
                    2,
                    101,
                    TradeSide::Sell,
                    "wallet-b",
                    1,
                    u64::MAX,
                    (None, None),
                ),
            ],
        )
        .expect("auxiliary price change must not wedge a valid window");

        let prices = snapshot
            .prices
            .expect("exact price points remain available");
        assert!(prices.change_bps.is_none());
        assert_eq!(snapshot.metrics.trades, 2);
        assert!(snapshot.completeness.is_complete());
    }
}
