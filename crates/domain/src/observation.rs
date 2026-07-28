use serde::{Deserialize, Serialize};

use crate::{MarketIdentity, Network};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Commitment {
    Processed,
    Confirmed,
    Finalized,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SourceProgram {
    Pump,
    PumpSwap,
    RaydiumCpmm,
    RaydiumClmm,
    RaydiumAmmV4,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TradeSide {
    Buy,
    Sell,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ChainCoordinate {
    pub slot: u64,
    /// Optional provider-supplied ordinal within a slot. Solana's standard RPC
    /// responses do not guarantee this value, so recovery must remain safe
    /// when it is absent.
    #[serde(default)]
    pub transaction_index: Option<u64>,
    pub signature: String,
    pub instruction_index: u16,
    pub event_index: u16,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ObservationKey {
    pub network: Network,
    pub program: SourceProgram,
    pub coordinate: ChainCoordinate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NormalizedObservation {
    pub key: ObservationKey,
    pub commitment: Commitment,
    pub schema_version: u16,
    pub decoder_version: String,
    pub event_kind: String,
    pub market: MarketIdentity,
    pub source_event_time_unix_ms: Option<i64>,
    pub received_time_unix_ms: i64,
    pub raw_evidence_hash: String,
    /// Compact base64 evidence sufficient to reproduce or audit the decode
    /// without retaining an entire RPC transaction response.
    #[serde(default)]
    pub source_evidence_base64: String,
    /// Complete versioned source event as decoded from the Pump public IDL.
    ///
    /// The stable source-neutral payload remains the cross-venue input. This
    /// record preserves protocol-specific facts such as fees, reserve variants,
    /// decimals, liquidity and mode flags for later feature extraction.
    #[serde(default)]
    pub source_details: serde_json::Value,
    pub payload: ObservationPayload,
}

/// Source-neutral facts needed by discovery and later screening. The source
/// decoder may expose additional protocol fields, but only these typed facts
/// cross the durable domain boundary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ObservationPayload {
    TokenCreated {
        name: String,
        symbol: String,
        uri: String,
        creator: String,
        user: String,
    },
    MarketCreated {
        creator: String,
        base_amount_units: u64,
        quote_amount_units: u64,
    },
    Trade {
        side: TradeSide,
        wallet: String,
        base_amount_units: u64,
        quote_amount_units: u64,
        base_reserve_units: Option<u64>,
        quote_reserve_units: Option<u64>,
    },
    /// Source-neutral liquidity addition after source mint ordering has been
    /// converted to the market's canonical token/quote orientation.
    LiquidityDeposited {
        provider: String,
        base_amount_units: u64,
        quote_amount_units: u64,
        base_reserve_units: u64,
        quote_reserve_units: u64,
        lp_token_amount_units: u64,
        lp_token_supply_units: u64,
    },
    /// Source-neutral liquidity removal after source mint ordering has been
    /// converted to the market's canonical token/quote orientation.
    LiquidityWithdrawn {
        provider: String,
        base_amount_units: u64,
        quote_amount_units: u64,
        base_reserve_units: u64,
        quote_reserve_units: u64,
        lp_token_amount_units: u64,
        lp_token_supply_units: u64,
    },
    MarketCompleted {
        user: String,
    },
    MarketMigrated {
        user: String,
        destination_market: String,
        base_amount_units: u64,
        /// Pump's legacy migration event names this amount as SOL. It must not
        /// be generalized to the active market's canonical quote units.
        legacy_sol_amount_units: u64,
    },
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::Network;

    use super::{ChainCoordinate, ObservationKey, ObservationPayload, SourceProgram};

    fn key(event_index: u16) -> ObservationKey {
        ObservationKey {
            network: Network::SolanaMainnet,
            program: SourceProgram::Pump,
            coordinate: ChainCoordinate {
                slot: 42,
                transaction_index: Some(3),
                signature: "signature".to_owned(),
                instruction_index: 1,
                event_index,
            },
        }
    }

    #[test]
    fn event_index_is_part_of_deduplication_identity() {
        let keys = HashSet::from([key(0), key(1)]);

        assert_eq!(keys.len(), 2);
    }

    #[test]
    fn network_is_part_of_deduplication_identity() {
        let mainnet = key(0);
        let mut devnet = mainnet.clone();
        devnet.network = Network::SolanaDevnet;

        assert_ne!(mainnet, devnet);
    }

    #[test]
    fn source_neutral_liquidity_payload_round_trips_exact_units() {
        let payload = ObservationPayload::LiquidityWithdrawn {
            provider: "provider".to_owned(),
            base_amount_units: u64::MAX,
            quote_amount_units: 2,
            base_reserve_units: 3,
            quote_reserve_units: 4,
            lp_token_amount_units: 5,
            lp_token_supply_units: 6,
        };
        let encoded = serde_json::to_value(&payload).expect("serialize liquidity");
        assert_eq!(encoded["type"], "LIQUIDITY_WITHDRAWN");
        assert_eq!(encoded["data"]["base_amount_units"], u64::MAX);
        let decoded: ObservationPayload =
            serde_json::from_value(encoded).expect("deserialize liquidity");
        assert_eq!(decoded, payload);
    }
}
