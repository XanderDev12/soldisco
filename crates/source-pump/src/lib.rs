//! Strict, IDL-derived Pump and PumpSwap event decoding.
//!
//! The decoder accepts only events attributed to the two supported programs,
//! preserves exact transaction coordinates supplied by the transaction
//! walker, and rejects malformed or unknown layouts. It performs no scoring,
//! filtering, or trading decisions.

mod decoder;
mod idl;
mod model;

pub use decoder::{
    DecodeError, decode_anchor_event, decode_cpi_event, decode_instruction_events,
    decode_instruction_program_data_logs, decode_program_data_bytes, decode_program_data_log,
    is_anchor_event_cpi,
};
pub use idl::{
    ANCHOR_EVENT_CPI_DISCRIMINATOR, COMPLETE_EVENT_DISCRIMINATOR,
    COMPLETE_PUMP_AMM_MIGRATION_EVENT_DISCRIMINATOR, CREATE_EVENT_DISCRIMINATOR,
    CREATE_POOL_EVENT_DISCRIMINATOR, DECODER_SCHEMA_VERSION, DECODER_VERSION, IDL_SOURCE_REVISION,
    PUMP_AMM_IDL_SOURCE, PUMP_IDL_SOURCE, PUMP_PROGRAM_ID, PUMP_SWAP_BUY_EVENT_DISCRIMINATOR,
    PUMP_SWAP_PROGRAM_ID, PUMP_SWAP_SELL_EVENT_DISCRIMINATOR, TRADE_EVENT_DISCRIMINATOR,
};
pub use model::{
    DecodedPumpEvent, PumpCompleteEvent, PumpCreateEvent, PumpEvent, PumpEventKind,
    PumpMigrationEvent, PumpProgram, PumpShareholder, PumpSwapBuyEvent, PumpSwapCreatePoolEvent,
    PumpSwapSellEvent, PumpTradeEvent, TradeDirection,
};

use soldisco_domain::SourceProgram;

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
