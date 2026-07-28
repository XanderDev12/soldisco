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

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize, Deserialize)]
pub struct ChainCoordinate {
    pub slot: u64,
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
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use crate::Network;

    use super::{ChainCoordinate, ObservationKey, SourceProgram};

    fn key(event_index: u16) -> ObservationKey {
        ObservationKey {
            network: Network::SolanaMainnet,
            program: SourceProgram::Pump,
            coordinate: ChainCoordinate {
                slot: 42,
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
}
