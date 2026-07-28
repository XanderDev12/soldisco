use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Network {
    SolanaMainnet,
    SolanaDevnet,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Venue {
    PumpBondingCurve,
    PumpSwap,
    RaydiumCpmm,
    RaydiumClmm,
    RaydiumAmmV4,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct MarketIdentity {
    pub network: Network,
    pub mint: String,
    pub venue: Venue,
    pub market_address: String,
    pub quote_mint: Option<String>,
}

impl MarketIdentity {
    #[must_use]
    pub fn is_raydium(&self) -> bool {
        matches!(
            self.venue,
            Venue::RaydiumCpmm | Venue::RaydiumClmm | Venue::RaydiumAmmV4
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{MarketIdentity, Network, Venue};

    #[test]
    fn identifies_raydium_without_treating_every_market_as_raydium() {
        let market = MarketIdentity {
            network: Network::SolanaMainnet,
            mint: "mint".to_owned(),
            venue: Venue::RaydiumCpmm,
            market_address: "pool".to_owned(),
            quote_mint: Some("quote".to_owned()),
        };

        assert!(market.is_raydium());
    }
}
