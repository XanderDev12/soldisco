use std::time::Duration;

use soldisco_api_contracts::{DiscoveryMode, DiscoveryStage};
use soldisco_domain::{AssessmentDecision, Commitment, Network, SourceProgram, Venue};

use crate::PersistenceError;

pub(crate) fn to_i64(value: u64, field: &'static str) -> Result<i64, PersistenceError> {
    i64::try_from(value).map_err(|_| PersistenceError::ValueOutOfRange { field })
}

pub(crate) fn to_u64(value: i64, field: &'static str) -> Result<u64, PersistenceError> {
    u64::try_from(value).map_err(|_| PersistenceError::InvalidStoredValue {
        field,
        value: value.to_string(),
    })
}

pub(crate) fn duration_millis(
    duration: Duration,
    field: &'static str,
    allow_zero: bool,
) -> Result<i64, PersistenceError> {
    if !allow_zero && duration.is_zero() {
        return Err(PersistenceError::MustBePositive { field });
    }
    i64::try_from(duration.as_millis()).map_err(|_| PersistenceError::ValueOutOfRange { field })
}

pub(crate) fn require_non_empty(value: &str, field: &'static str) -> Result<(), PersistenceError> {
    if value.trim().is_empty() {
        Err(PersistenceError::EmptyField { field })
    } else {
        Ok(())
    }
}

pub(crate) const fn network_name(network: Network) -> &'static str {
    match network {
        Network::SolanaMainnet => "SOLANA_MAINNET",
        Network::SolanaDevnet => "SOLANA_DEVNET",
    }
}

pub(crate) fn parse_network(value: &str) -> Result<Network, PersistenceError> {
    match value {
        "SOLANA_MAINNET" => Ok(Network::SolanaMainnet),
        "SOLANA_DEVNET" => Ok(Network::SolanaDevnet),
        _ => Err(PersistenceError::InvalidStoredValue {
            field: "network",
            value: value.to_owned(),
        }),
    }
}

pub(crate) const fn source_program_name(program: SourceProgram) -> &'static str {
    match program {
        SourceProgram::Pump => "PUMP",
        SourceProgram::PumpSwap => "PUMP_SWAP",
        SourceProgram::RaydiumCpmm => "RAYDIUM_CPMM",
        SourceProgram::RaydiumClmm => "RAYDIUM_CLMM",
        SourceProgram::RaydiumAmmV4 => "RAYDIUM_AMM_V4",
    }
}

pub(crate) fn parse_source_program(value: &str) -> Result<SourceProgram, PersistenceError> {
    match value {
        "PUMP" => Ok(SourceProgram::Pump),
        "PUMP_SWAP" => Ok(SourceProgram::PumpSwap),
        "RAYDIUM_CPMM" => Ok(SourceProgram::RaydiumCpmm),
        "RAYDIUM_CLMM" => Ok(SourceProgram::RaydiumClmm),
        "RAYDIUM_AMM_V4" => Ok(SourceProgram::RaydiumAmmV4),
        _ => Err(PersistenceError::InvalidStoredValue {
            field: "source_program",
            value: value.to_owned(),
        }),
    }
}

pub(crate) const fn commitment_name(commitment: Commitment) -> &'static str {
    match commitment {
        Commitment::Processed => "PROCESSED",
        Commitment::Confirmed => "CONFIRMED",
        Commitment::Finalized => "FINALIZED",
    }
}

pub(crate) const fn venue_name(venue: Venue) -> &'static str {
    match venue {
        Venue::PumpBondingCurve => "PUMP_BONDING_CURVE",
        Venue::PumpSwap => "PUMP_SWAP",
        Venue::RaydiumCpmm => "RAYDIUM_CPMM",
        Venue::RaydiumClmm => "RAYDIUM_CLMM",
        Venue::RaydiumAmmV4 => "RAYDIUM_AMM_V4",
    }
}

pub(crate) fn parse_venue(value: &str) -> Result<Venue, PersistenceError> {
    match value {
        "PUMP_BONDING_CURVE" => Ok(Venue::PumpBondingCurve),
        "PUMP_SWAP" => Ok(Venue::PumpSwap),
        "RAYDIUM_CPMM" => Ok(Venue::RaydiumCpmm),
        "RAYDIUM_CLMM" => Ok(Venue::RaydiumClmm),
        "RAYDIUM_AMM_V4" => Ok(Venue::RaydiumAmmV4),
        _ => Err(PersistenceError::InvalidStoredValue {
            field: "venue",
            value: value.to_owned(),
        }),
    }
}

pub(crate) fn discovery_mode_name(mode: DiscoveryMode) -> &'static str {
    match mode {
        DiscoveryMode::ObserveAll => "OBSERVE_ALL",
        DiscoveryMode::ApprovedOnly => "APPROVED_ONLY",
    }
}

pub(crate) fn parse_discovery_mode(value: &str) -> Result<DiscoveryMode, PersistenceError> {
    match value {
        "OBSERVE_ALL" => Ok(DiscoveryMode::ObserveAll),
        "APPROVED_ONLY" => Ok(DiscoveryMode::ApprovedOnly),
        _ => Err(PersistenceError::InvalidStoredValue {
            field: "discovery_projection_state.mode",
            value: value.to_owned(),
        }),
    }
}

pub(crate) fn discovery_stage_name(stage: DiscoveryStage) -> &'static str {
    match stage {
        DiscoveryStage::Observed => "OBSERVED",
        DiscoveryStage::Approved => "APPROVED",
    }
}

pub(crate) const fn assessment_decision_name(decision: AssessmentDecision) -> &'static str {
    match decision {
        AssessmentDecision::Pass => "PASS",
        AssessmentDecision::Reject => "REJECT",
        AssessmentDecision::Unknown => "UNKNOWN",
    }
}
