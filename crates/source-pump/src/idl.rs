//! Constants derived from Pump's public Anchor IDLs.
//!
//! Source revision:
//! <https://github.com/pump-fun/pump-public-docs/tree/9c82f61cb711b044a17f770ab8ce9f9bdf78f333/idl>

use crate::PumpProgram;

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

// These complete discriminator sets are pinned to the public IDL revision
// above. Soldisco decodes only the market events it consumes, but it must be
// able to distinguish a known, intentionally ignored event from an event that
// is absent from the pinned schema.
const PUMP_EVENT_DISCRIMINATORS: [[u8; 8]; 23] = [
    [64, 69, 192, 104, 29, 30, 25, 107],
    [245, 59, 70, 34, 75, 185, 109, 92],
    [147, 250, 108, 120, 247, 29, 67, 222],
    [226, 214, 246, 33, 7, 242, 147, 229],
    [79, 172, 246, 49, 205, 91, 206, 232],
    [146, 159, 189, 172, 146, 88, 56, 244],
    [122, 2, 127, 1, 14, 191, 12, 175],
    COMPLETE_EVENT_DISCRIMINATOR,
    COMPLETE_PUMP_AMM_MIGRATION_EVENT_DISCRIMINATOR,
    CREATE_EVENT_DISCRIMINATOR,
    [165, 55, 129, 112, 4, 179, 202, 40],
    [97, 97, 215, 144, 93, 146, 22, 124],
    [134, 36, 13, 72, 232, 101, 130, 216],
    [155, 167, 104, 220, 213, 108, 243, 3],
    [168, 216, 132, 239, 235, 182, 49, 52],
    [43, 188, 250, 18, 221, 75, 187, 95],
    [237, 52, 123, 37, 245, 251, 72, 210],
    [142, 203, 6, 32, 127, 105, 191, 162],
    [223, 195, 159, 246, 62, 48, 143, 131],
    [197, 122, 167, 124, 116, 81, 91, 255],
    TRADE_EVENT_DISCRIMINATOR,
    [182, 195, 137, 42, 35, 206, 207, 247],
    [117, 123, 228, 182, 161, 168, 220, 214],
];

const PUMP_SWAP_EVENT_DISCRIMINATORS: [[u8; 8]; 25] = [
    [45, 220, 93, 24, 25, 97, 172, 104],
    [147, 250, 108, 120, 247, 29, 67, 222],
    [63, 69, 28, 22, 48, 92, 194, 185],
    PUMP_SWAP_BUY_EVENT_DISCRIMINATOR,
    [226, 214, 246, 33, 7, 242, 147, 229],
    [79, 172, 246, 49, 205, 91, 206, 232],
    [146, 159, 189, 172, 146, 88, 56, 244],
    [232, 245, 194, 238, 234, 218, 58, 89],
    [107, 52, 89, 129, 55, 226, 81, 22],
    CREATE_POOL_EVENT_DISCRIMINATOR,
    [120, 248, 61, 83, 31, 142, 107, 144],
    [107, 253, 193, 76, 228, 202, 27, 104],
    [97, 97, 215, 144, 93, 146, 22, 124],
    [174, 124, 74, 249, 4, 81, 246, 17],
    [134, 36, 13, 72, 232, 101, 130, 216],
    [170, 221, 82, 199, 147, 165, 247, 46],
    [43, 188, 250, 18, 221, 75, 187, 95],
    PUMP_SWAP_SELL_EVENT_DISCRIMINATOR,
    [242, 231, 235, 102, 65, 99, 189, 211],
    [89, 128, 240, 141, 91, 202, 71, 105],
    [150, 107, 199, 123, 124, 207, 102, 228],
    [197, 122, 167, 124, 116, 81, 91, 255],
    [225, 152, 171, 87, 246, 63, 66, 234],
    [90, 23, 65, 35, 62, 244, 188, 208],
    [22, 9, 133, 26, 160, 44, 71, 192],
];

/// Returns whether an event discriminator belongs to the complete pinned
/// public IDL for the selected program.
#[must_use]
pub fn is_pinned_event_discriminator(program: PumpProgram, discriminator: [u8; 8]) -> bool {
    match program {
        PumpProgram::Pump => PUMP_EVENT_DISCRIMINATORS.contains(&discriminator),
        PumpProgram::PumpSwap => PUMP_SWAP_EVENT_DISCRIMINATORS.contains(&discriminator),
    }
}
