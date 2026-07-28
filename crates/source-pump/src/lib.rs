//! Pump and PumpSwap source boundary.
//!
//! The next collector milestone will add verified IDL-backed binary decoders.
//! This foundation fixes the supported program identities and the normalized
//! event envelope without pretending that live decoding already exists.

use serde::{Deserialize, Serialize};
use soldisco_domain::{ChainCoordinate, MarketIdentity, SourceProgram};

pub const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const PUMP_SWAP_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PumpProgram {
    Pump,
    PumpSwap,
}

impl PumpProgram {
    #[must_use]
    pub const fn program_id(self) -> &'static str {
        match self {
            Self::Pump => PUMP_PROGRAM_ID,
            Self::PumpSwap => PUMP_SWAP_PROGRAM_ID,
        }
    }

    #[must_use]
    pub const fn source_program(self) -> SourceProgram {
        match self {
            Self::Pump => SourceProgram::Pump,
            Self::PumpSwap => SourceProgram::PumpSwap,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PumpEventKind {
    Create,
    Trade,
    Complete,
    CompletePumpAmmMigration,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PumpObservation {
    pub program: PumpProgram,
    pub event_kind: PumpEventKind,
    pub coordinate: ChainCoordinate,
    pub market: MarketIdentity,
}

#[must_use]
pub fn supported_program(program_id: &str) -> Option<PumpProgram> {
    match program_id {
        PUMP_PROGRAM_ID => Some(PumpProgram::Pump),
        PUMP_SWAP_PROGRAM_ID => Some(PumpProgram::PumpSwap),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{PUMP_PROGRAM_ID, PUMP_SWAP_PROGRAM_ID, PumpProgram, supported_program};

    #[test]
    fn supported_program_ids_are_distinct_and_recognized() {
        assert_ne!(PUMP_PROGRAM_ID, PUMP_SWAP_PROGRAM_ID);
        assert_eq!(supported_program(PUMP_PROGRAM_ID), Some(PumpProgram::Pump));
        assert_eq!(
            supported_program(PUMP_SWAP_PROGRAM_ID),
            Some(PumpProgram::PumpSwap)
        );
        assert_eq!(supported_program("unknown"), None);
    }
}
