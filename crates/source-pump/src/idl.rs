//! Constants derived from Pump's public Anchor IDLs.
//!
//! Source revision:
//! <https://github.com/pump-fun/pump-public-docs/tree/9c82f61cb711b044a17f770ab8ce9f9bdf78f333/idl>

pub const PUMP_PROGRAM_ID: &str = "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P";
pub const PUMP_SWAP_PROGRAM_ID: &str = "pAMMBay6oceH9fJKBRHGP5D4bD4sWpmSwMn52FMfXEA";

pub const IDL_SOURCE_REVISION: &str = "9c82f61cb711b044a17f770ab8ce9f9bdf78f333";
pub const PUMP_IDL_SOURCE: &str = "https://github.com/pump-fun/pump-public-docs/blob/9c82f61cb711b044a17f770ab8ce9f9bdf78f333/idl/pump.json";
pub const PUMP_AMM_IDL_SOURCE: &str = "https://github.com/pump-fun/pump-public-docs/blob/9c82f61cb711b044a17f770ab8ce9f9bdf78f333/idl/pump_amm.json";

pub const DECODER_SCHEMA_VERSION: u16 = 1;
pub const DECODER_VERSION: &str = concat!(
    "pump-idl-9c82f61cb711b044a17f770ab8ce9f9bdf78f333-soldisco-",
    env!("CARGO_PKG_VERSION")
);

/// Anchor's `emit_cpi!` instruction prefix. Direct `Program data:` events do
/// not include this prefix.
pub const ANCHOR_EVENT_CPI_DISCRIMINATOR: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];

pub const COMPLETE_EVENT_DISCRIMINATOR: [u8; 8] = [95, 114, 97, 156, 212, 46, 152, 8];
pub const COMPLETE_PUMP_AMM_MIGRATION_EVENT_DISCRIMINATOR: [u8; 8] =
    [189, 233, 93, 185, 92, 148, 234, 148];
pub const CREATE_EVENT_DISCRIMINATOR: [u8; 8] = [27, 114, 169, 77, 222, 235, 99, 118];
pub const TRADE_EVENT_DISCRIMINATOR: [u8; 8] = [189, 219, 127, 211, 78, 230, 97, 238];

pub const CREATE_POOL_EVENT_DISCRIMINATOR: [u8; 8] = [177, 49, 12, 210, 160, 118, 167, 116];
pub const PUMP_SWAP_BUY_EVENT_DISCRIMINATOR: [u8; 8] = [103, 244, 82, 31, 44, 245, 119, 119];
pub const PUMP_SWAP_SELL_EVENT_DISCRIMINATOR: [u8; 8] = [62, 47, 55, 10, 165, 3, 220, 42];
