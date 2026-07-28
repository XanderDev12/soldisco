use std::collections::HashSet;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TradeSide {
    Buy,
    Sell,
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
    use super::{
        Qualification, QualificationRules, RollingMarketMetrics, TradeObservation, TradeSide,
    };

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
