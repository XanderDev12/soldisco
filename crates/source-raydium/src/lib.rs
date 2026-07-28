//! Optional Raydium venue evidence for candidates first discovered through
//! Pump or PumpSwap.

use serde::{Deserialize, Serialize};
use soldisco_domain::{MarketIdentity, SourceProgram, Venue};

pub const RAYDIUM_AMM_V4_PROGRAM_ID: &str = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8";
pub const RAYDIUM_CPMM_PROGRAM_ID: &str = "CPMMoo8L3F4NbTegBCKVNunggL7H1ZpdTHKxQB5qKP1C";
pub const RAYDIUM_CLMM_PROGRAM_ID: &str = "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RaydiumProgram {
    Cpmm,
    Clmm,
    AmmV4,
}

impl RaydiumProgram {
    #[must_use]
    pub const fn program_id(self) -> &'static str {
        match self {
            Self::Cpmm => RAYDIUM_CPMM_PROGRAM_ID,
            Self::Clmm => RAYDIUM_CLMM_PROGRAM_ID,
            Self::AmmV4 => RAYDIUM_AMM_V4_PROGRAM_ID,
        }
    }

    #[must_use]
    pub const fn venue(self) -> Venue {
        match self {
            Self::Cpmm => Venue::RaydiumCpmm,
            Self::Clmm => Venue::RaydiumClmm,
            Self::AmmV4 => Venue::RaydiumAmmV4,
        }
    }

    #[must_use]
    pub const fn source_program(self) -> SourceProgram {
        match self {
            Self::Cpmm => SourceProgram::RaydiumCpmm,
            Self::Clmm => SourceProgram::RaydiumClmm,
            Self::AmmV4 => SourceProgram::RaydiumAmmV4,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RaydiumPoolEvidence {
    pub program: RaydiumProgram,
    pub market: MarketIdentity,
    pub observed_slot: u64,
    pub liquidity_quote_units: Option<u128>,
    pub recent_volume_quote_units: Option<u128>,
}

/// Raydium is an independent enrichment state. In particular,
/// `NoSupportedPool` is not equivalent to a failed deterministic safety rule.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    content = "details",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum RaydiumEnrichment {
    NotChecked,
    NoSupportedPool,
    Pools(Vec<RaydiumPoolEvidence>),
    Unavailable { reason_code: String },
}

impl RaydiumEnrichment {
    #[must_use]
    pub const fn is_rejection(&self) -> bool {
        false
    }
}

#[must_use]
pub fn supported_program(program_id: &str) -> Option<RaydiumProgram> {
    match program_id {
        RAYDIUM_CPMM_PROGRAM_ID => Some(RaydiumProgram::Cpmm),
        RAYDIUM_CLMM_PROGRAM_ID => Some(RaydiumProgram::Clmm),
        RAYDIUM_AMM_V4_PROGRAM_ID => Some(RaydiumProgram::AmmV4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{
        RAYDIUM_AMM_V4_PROGRAM_ID, RAYDIUM_CLMM_PROGRAM_ID, RAYDIUM_CPMM_PROGRAM_ID,
        RaydiumEnrichment, RaydiumProgram, supported_program,
    };

    #[test]
    fn supported_program_ids_are_distinct_and_recognized() {
        let ids = [
            RAYDIUM_AMM_V4_PROGRAM_ID,
            RAYDIUM_CPMM_PROGRAM_ID,
            RAYDIUM_CLMM_PROGRAM_ID,
        ];

        assert_eq!(ids.into_iter().collect::<HashSet<_>>().len(), 3);
        assert_eq!(
            supported_program(RAYDIUM_CPMM_PROGRAM_ID),
            Some(RaydiumProgram::Cpmm)
        );
        assert_eq!(supported_program("unknown"), None);
    }

    #[test]
    fn absence_of_a_pool_is_not_a_rejection() {
        assert!(!RaydiumEnrichment::NoSupportedPool.is_rejection());
    }
}
