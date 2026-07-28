use base64::{Engine as _, engine::general_purpose::STANDARD};
use soldisco_domain::ChainCoordinate;
use thiserror::Error;

use crate::{
    ANCHOR_EVENT_CPI_DISCRIMINATOR, COMPLETE_EVENT_DISCRIMINATOR,
    COMPLETE_PUMP_AMM_MIGRATION_EVENT_DISCRIMINATOR, CREATE_EVENT_DISCRIMINATOR,
    CREATE_POOL_EVENT_DISCRIMINATOR, DecodedPumpEvent, PUMP_SWAP_BUY_EVENT_DISCRIMINATOR,
    PUMP_SWAP_SELL_EVENT_DISCRIMINATOR, PumpCompleteEvent, PumpCreateEvent, PumpEvent,
    PumpMigrationEvent, PumpProgram, PumpShareholder, PumpSwapBuyEvent, PumpSwapCreatePoolEvent,
    PumpSwapDepositEvent, PumpSwapSellEvent, PumpSwapWithdrawEvent, PumpTradeEvent,
    TRADE_EVENT_DISCRIMINATOR, TradeDirection, supported_program,
};

const PROGRAM_DATA_PREFIX: &str = "Program data: ";
const MAX_NAME_BYTES: usize = 256;
const MAX_SYMBOL_BYTES: usize = 64;
const MAX_URI_BYTES: usize = 4_096;
const MAX_INSTRUCTION_NAME_BYTES: usize = 96;
const MAX_SHAREHOLDERS: usize = 64;

/// PumpSwap `DepositEvent` discriminator from the pinned public IDL revision.
pub const PUMP_SWAP_DEPOSIT_EVENT_DISCRIMINATOR: [u8; 8] = [120, 248, 61, 83, 31, 142, 107, 144];
/// PumpSwap `WithdrawEvent` discriminator from the pinned public IDL revision.
pub const PUMP_SWAP_WITHDRAW_EVENT_DISCRIMINATOR: [u8; 8] = [22, 9, 133, 26, 160, 44, 71, 192];

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("unsupported Pump source program: {0}")]
    UnsupportedProgram(String),
    #[error("transaction signature cannot be empty")]
    EmptySignature,
    #[error("event payload is shorter than the 8-byte Anchor discriminator")]
    MissingDiscriminator,
    #[error("inner instruction is not an Anchor event CPI")]
    MissingCpiDiscriminator,
    #[error("unknown event discriminator {discriminator:?} for {program:?}")]
    UnknownDiscriminator {
        program: PumpProgram,
        discriminator: [u8; 8],
    },
    #[error("event field `{field}` is truncated: needs {needed} bytes, has {remaining}")]
    Truncated {
        field: &'static str,
        needed: usize,
        remaining: usize,
    },
    #[error("event field `{field}` contains invalid UTF-8")]
    InvalidUtf8 { field: &'static str },
    #[error("event field `{field}` has invalid boolean byte {value}")]
    InvalidBoolean { field: &'static str, value: u8 },
    #[error("event field `{field}` length {actual} exceeds limit {limit}")]
    LimitExceeded {
        field: &'static str,
        limit: usize,
        actual: usize,
    },
    #[error("event has {0} unexpected trailing bytes")]
    TrailingBytes(usize),
    #[error("expected an Anchor `Program data:` log")]
    InvalidProgramDataLog,
    #[error("invalid base64 in `Program data:` log")]
    InvalidBase64(#[source] base64::DecodeError),
    #[error("an instruction emitted more than {0} events")]
    TooManyEvents(u16),
}

#[must_use]
pub fn is_anchor_event_cpi(data: &[u8]) -> bool {
    data.starts_with(&ANCHOR_EVENT_CPI_DISCRIMINATOR)
}

/// Decode an authoritative Anchor `emit_cpi!` inner instruction.
///
/// Unlike [`decode_anchor_event`], this entry point requires the event CPI
/// prefix, preventing an ordinary Pump instruction from being mistaken for an
/// event.
pub fn decode_cpi_event(
    program_id: &str,
    coordinate: ChainCoordinate,
    data: &[u8],
) -> Result<DecodedPumpEvent, DecodeError> {
    if !is_anchor_event_cpi(data) {
        return Err(DecodeError::MissingCpiDiscriminator);
    }
    decode_anchor_event(program_id, coordinate, data)
}

/// Decode one direct Anchor event payload or one Anchor `emit_cpi!` payload.
///
/// The transaction walker is responsible for attributing `program_id` and
/// supplying the exact instruction and event indexes. Unknown or malformed
/// payloads return an error and never produce a partial event.
pub fn decode_anchor_event(
    program_id: &str,
    coordinate: ChainCoordinate,
    data: &[u8],
) -> Result<DecodedPumpEvent, DecodeError> {
    let program = supported_program(program_id)
        .ok_or_else(|| DecodeError::UnsupportedProgram(program_id.to_owned()))?;
    if coordinate.signature.is_empty() {
        return Err(DecodeError::EmptySignature);
    }

    let event_data = data
        .strip_prefix(&ANCHOR_EVENT_CPI_DISCRIMINATOR)
        .unwrap_or(data);
    let discriminator: [u8; 8] = event_data
        .get(..8)
        .ok_or(DecodeError::MissingDiscriminator)?
        .try_into()
        .expect("slice length is checked");
    let mut reader = BorshReader::new(&event_data[8..]);

    let event = match (program, discriminator) {
        (PumpProgram::Pump, CREATE_EVENT_DISCRIMINATOR) => {
            PumpEvent::Create(decode_pump_create(&mut reader)?)
        }
        (PumpProgram::Pump, TRADE_EVENT_DISCRIMINATOR) => {
            PumpEvent::Trade(decode_pump_trade(&mut reader)?)
        }
        (PumpProgram::Pump, COMPLETE_EVENT_DISCRIMINATOR) => {
            PumpEvent::Complete(decode_pump_complete(&mut reader)?)
        }
        (PumpProgram::Pump, COMPLETE_PUMP_AMM_MIGRATION_EVENT_DISCRIMINATOR) => {
            PumpEvent::CompletePumpAmmMigration(decode_pump_migration(&mut reader)?)
        }
        (PumpProgram::PumpSwap, CREATE_POOL_EVENT_DISCRIMINATOR) => {
            PumpEvent::PumpSwapCreatePool(decode_pump_swap_create_pool(&mut reader)?)
        }
        (PumpProgram::PumpSwap, PUMP_SWAP_BUY_EVENT_DISCRIMINATOR) => {
            PumpEvent::PumpSwapBuy(decode_pump_swap_buy(&mut reader)?)
        }
        (PumpProgram::PumpSwap, PUMP_SWAP_SELL_EVENT_DISCRIMINATOR) => {
            PumpEvent::PumpSwapSell(decode_pump_swap_sell(&mut reader)?)
        }
        (PumpProgram::PumpSwap, PUMP_SWAP_DEPOSIT_EVENT_DISCRIMINATOR) => {
            PumpEvent::PumpSwapDeposit(decode_pump_swap_deposit(&mut reader)?)
        }
        (PumpProgram::PumpSwap, PUMP_SWAP_WITHDRAW_EVENT_DISCRIMINATOR) => {
            PumpEvent::PumpSwapWithdraw(decode_pump_swap_withdraw(&mut reader)?)
        }
        _ => {
            return Err(DecodeError::UnknownDiscriminator {
                program,
                discriminator,
            });
        }
    };

    reader.finish()?;
    Ok(DecodedPumpEvent {
        program,
        coordinate,
        event,
    })
}

/// Decode one Anchor `Program data: <base64>` log.
pub fn decode_program_data_log(
    program_id: &str,
    coordinate: ChainCoordinate,
    log: &str,
) -> Result<DecodedPumpEvent, DecodeError> {
    let data = decode_program_data_bytes(log)?;
    decode_anchor_event(program_id, coordinate, &data)
}

/// Returns the exact decoded bytes carried by an Anchor `Program data:` log.
///
/// Persistence uses this compact payload as replay/audit evidence instead of
/// retaining the complete RPC transaction response or the textual base64
/// wrapper.
pub fn decode_program_data_bytes(log: &str) -> Result<Vec<u8>, DecodeError> {
    let encoded = log
        .strip_prefix(PROGRAM_DATA_PREFIX)
        .ok_or(DecodeError::InvalidProgramDataLog)?;
    STANDARD.decode(encoded).map_err(DecodeError::InvalidBase64)
}

/// Decode multiple binary events emitted by one instruction.
///
/// `event_index` is assigned in payload order, starting at zero.
pub fn decode_instruction_events(
    program_id: &str,
    slot: u64,
    signature: &str,
    instruction_index: u16,
    payloads: &[&[u8]],
) -> Result<Vec<DecodedPumpEvent>, DecodeError> {
    payloads
        .iter()
        .enumerate()
        .map(|(event_index, payload)| {
            let event_index =
                u16::try_from(event_index).map_err(|_| DecodeError::TooManyEvents(u16::MAX))?;
            decode_anchor_event(
                program_id,
                ChainCoordinate {
                    slot,
                    transaction_index: None,
                    signature: signature.to_owned(),
                    instruction_index,
                    event_index,
                },
                payload,
            )
        })
        .collect()
}

/// Decode all `Program data:` records from logs already scoped to one program
/// instruction invocation. Other log lines are ignored. A malformed
/// `Program data:` record fails the complete batch.
pub fn decode_instruction_program_data_logs(
    program_id: &str,
    slot: u64,
    signature: &str,
    instruction_index: u16,
    logs: &[&str],
) -> Result<Vec<DecodedPumpEvent>, DecodeError> {
    let mut events = Vec::new();
    for log in logs {
        if !log.starts_with(PROGRAM_DATA_PREFIX) {
            continue;
        }
        let event_index =
            u16::try_from(events.len()).map_err(|_| DecodeError::TooManyEvents(u16::MAX))?;
        events.push(decode_program_data_log(
            program_id,
            ChainCoordinate {
                slot,
                transaction_index: None,
                signature: signature.to_owned(),
                instruction_index,
                event_index,
            },
            log,
        )?);
    }
    Ok(events)
}

fn decode_pump_create(reader: &mut BorshReader<'_>) -> Result<PumpCreateEvent, DecodeError> {
    Ok(PumpCreateEvent {
        name: reader.string("name", MAX_NAME_BYTES)?,
        symbol: reader.string("symbol", MAX_SYMBOL_BYTES)?,
        uri: reader.string("uri", MAX_URI_BYTES)?,
        mint: reader.pubkey("mint")?,
        bonding_curve: reader.pubkey("bonding_curve")?,
        user: reader.pubkey("user")?,
        creator: reader.pubkey("creator")?,
        timestamp: reader.i64("timestamp")?,
        virtual_token_reserves: reader.u64("virtual_token_reserves")?,
        virtual_sol_reserves: reader.u64("virtual_sol_reserves")?,
        real_token_reserves: reader.u64("real_token_reserves")?,
        token_total_supply: reader.u64("token_total_supply")?,
        token_program: reader.pubkey("token_program")?,
        is_mayhem_mode: reader.boolean("is_mayhem_mode")?,
        is_cashback_enabled: reader.boolean("is_cashback_enabled")?,
        quote_mint: reader.pubkey("quote_mint")?,
        virtual_quote_reserves: reader.u64("virtual_quote_reserves")?,
    })
}

fn decode_pump_trade(reader: &mut BorshReader<'_>) -> Result<PumpTradeEvent, DecodeError> {
    let mint = reader.pubkey("mint")?;
    let legacy_sol_amount = reader.u64("sol_amount")?;
    let token_amount = reader.u64("token_amount")?;
    let direction = if reader.boolean("is_buy")? {
        TradeDirection::Buy
    } else {
        TradeDirection::Sell
    };

    Ok(PumpTradeEvent {
        mint,
        legacy_sol_amount,
        token_amount,
        direction,
        user: reader.pubkey("user")?,
        timestamp: reader.i64("timestamp")?,
        virtual_sol_reserves: reader.u64("virtual_sol_reserves")?,
        virtual_token_reserves: reader.u64("virtual_token_reserves")?,
        real_sol_reserves: reader.u64("real_sol_reserves")?,
        real_token_reserves: reader.u64("real_token_reserves")?,
        fee_recipient: reader.pubkey("fee_recipient")?,
        fee_basis_points: reader.u64("fee_basis_points")?,
        fee_amount: reader.u64("fee")?,
        creator: reader.pubkey("creator")?,
        creator_fee_basis_points: reader.u64("creator_fee_basis_points")?,
        creator_fee_amount: reader.u64("creator_fee")?,
        track_volume: reader.boolean("track_volume")?,
        total_unclaimed_tokens: reader.u64("total_unclaimed_tokens")?,
        total_claimed_tokens: reader.u64("total_claimed_tokens")?,
        current_sol_volume: reader.u64("current_sol_volume")?,
        last_update_timestamp: reader.i64("last_update_timestamp")?,
        instruction_name: reader.string("ix_name", MAX_INSTRUCTION_NAME_BYTES)?,
        mayhem_mode: reader.boolean("mayhem_mode")?,
        cashback_fee_basis_points: reader.u64("cashback_fee_basis_points")?,
        cashback_amount: reader.u64("cashback")?,
        buyback_fee_basis_points: reader.u64("buyback_fee_basis_points")?,
        buyback_fee_amount: reader.u64("buyback_fee")?,
        shareholders: reader.shareholders()?,
        quote_mint: reader.pubkey("quote_mint")?,
        quote_amount: reader.u64("quote_amount")?,
        virtual_quote_reserves: reader.u64("virtual_quote_reserves")?,
        real_quote_reserves: reader.u64("real_quote_reserves")?,
    })
}

fn decode_pump_complete(reader: &mut BorshReader<'_>) -> Result<PumpCompleteEvent, DecodeError> {
    Ok(PumpCompleteEvent {
        user: reader.pubkey("user")?,
        mint: reader.pubkey("mint")?,
        bonding_curve: reader.pubkey("bonding_curve")?,
        timestamp: reader.i64("timestamp")?,
        quote_mint: reader.pubkey("quote_mint")?,
    })
}

fn decode_pump_migration(reader: &mut BorshReader<'_>) -> Result<PumpMigrationEvent, DecodeError> {
    Ok(PumpMigrationEvent {
        user: reader.pubkey("user")?,
        mint: reader.pubkey("mint")?,
        mint_amount: reader.u64("mint_amount")?,
        legacy_sol_amount: reader.u64("sol_amount")?,
        pool_migration_fee: reader.u64("pool_migration_fee")?,
        bonding_curve: reader.pubkey("bonding_curve")?,
        timestamp: reader.i64("timestamp")?,
        pool: reader.pubkey("pool")?,
        quote_mint: reader.pubkey("quote_mint")?,
    })
}

fn decode_pump_swap_create_pool(
    reader: &mut BorshReader<'_>,
) -> Result<PumpSwapCreatePoolEvent, DecodeError> {
    Ok(PumpSwapCreatePoolEvent {
        timestamp: reader.i64("timestamp")?,
        index: reader.u16("index")?,
        creator: reader.pubkey("creator")?,
        base_mint: reader.pubkey("base_mint")?,
        quote_mint: reader.pubkey("quote_mint")?,
        base_mint_decimals: reader.u8("base_mint_decimals")?,
        quote_mint_decimals: reader.u8("quote_mint_decimals")?,
        base_amount_in: reader.u64("base_amount_in")?,
        quote_amount_in: reader.u64("quote_amount_in")?,
        pool_base_amount: reader.u64("pool_base_amount")?,
        pool_quote_amount: reader.u64("pool_quote_amount")?,
        minimum_liquidity: reader.u64("minimum_liquidity")?,
        initial_liquidity: reader.u64("initial_liquidity")?,
        lp_token_amount_out: reader.u64("lp_token_amount_out")?,
        pool_bump: reader.u8("pool_bump")?,
        pool: reader.pubkey("pool")?,
        lp_mint: reader.pubkey("lp_mint")?,
        user_base_token_account: reader.pubkey("user_base_token_account")?,
        user_quote_token_account: reader.pubkey("user_quote_token_account")?,
        coin_creator: reader.pubkey("coin_creator")?,
        is_mayhem_mode: reader.boolean("is_mayhem_mode")?,
    })
}

fn decode_pump_swap_buy(reader: &mut BorshReader<'_>) -> Result<PumpSwapBuyEvent, DecodeError> {
    Ok(PumpSwapBuyEvent {
        timestamp: reader.i64("timestamp")?,
        base_amount_out: reader.u64("base_amount_out")?,
        max_quote_amount_in: reader.u64("max_quote_amount_in")?,
        user_base_token_reserves: reader.u64("user_base_token_reserves")?,
        user_quote_token_reserves: reader.u64("user_quote_token_reserves")?,
        pool_base_token_reserves: reader.u64("pool_base_token_reserves")?,
        pool_quote_token_reserves: reader.u64("pool_quote_token_reserves")?,
        quote_amount_in: reader.u64("quote_amount_in")?,
        lp_fee_basis_points: reader.u64("lp_fee_basis_points")?,
        lp_fee_amount: reader.u64("lp_fee")?,
        protocol_fee_basis_points: reader.u64("protocol_fee_basis_points")?,
        protocol_fee_amount: reader.u64("protocol_fee")?,
        quote_amount_in_with_lp_fee: reader.u64("quote_amount_in_with_lp_fee")?,
        user_quote_amount_in: reader.u64("user_quote_amount_in")?,
        pool: reader.pubkey("pool")?,
        user: reader.pubkey("user")?,
        user_base_token_account: reader.pubkey("user_base_token_account")?,
        user_quote_token_account: reader.pubkey("user_quote_token_account")?,
        protocol_fee_recipient: reader.pubkey("protocol_fee_recipient")?,
        protocol_fee_recipient_token_account: reader
            .pubkey("protocol_fee_recipient_token_account")?,
        coin_creator: reader.pubkey("coin_creator")?,
        coin_creator_fee_basis_points: reader.u64("coin_creator_fee_basis_points")?,
        coin_creator_fee_amount: reader.u64("coin_creator_fee")?,
        track_volume: reader.boolean("track_volume")?,
        total_unclaimed_tokens: reader.u64("total_unclaimed_tokens")?,
        total_claimed_tokens: reader.u64("total_claimed_tokens")?,
        current_sol_volume: reader.u64("current_sol_volume")?,
        last_update_timestamp: reader.i64("last_update_timestamp")?,
        min_base_amount_out: reader.u64("min_base_amount_out")?,
        instruction_name: reader.string("ix_name", MAX_INSTRUCTION_NAME_BYTES)?,
        cashback_fee_basis_points: reader.u64("cashback_fee_basis_points")?,
        cashback_amount: reader.u64("cashback")?,
        buyback_fee_basis_points: reader.u64("buyback_fee_basis_points")?,
        buyback_fee_amount: reader.u64("buyback_fee")?,
        virtual_quote_reserves: reader.i128("virtual_quote_reserves")?,
        can_boost: reader.boolean("can_boost")?,
        base_supply: reader.u64("base_supply")?,
    })
}

fn decode_pump_swap_sell(reader: &mut BorshReader<'_>) -> Result<PumpSwapSellEvent, DecodeError> {
    Ok(PumpSwapSellEvent {
        timestamp: reader.i64("timestamp")?,
        base_amount_in: reader.u64("base_amount_in")?,
        min_quote_amount_out: reader.u64("min_quote_amount_out")?,
        user_base_token_reserves: reader.u64("user_base_token_reserves")?,
        user_quote_token_reserves: reader.u64("user_quote_token_reserves")?,
        pool_base_token_reserves: reader.u64("pool_base_token_reserves")?,
        pool_quote_token_reserves: reader.u64("pool_quote_token_reserves")?,
        quote_amount_out: reader.u64("quote_amount_out")?,
        lp_fee_basis_points: reader.u64("lp_fee_basis_points")?,
        lp_fee_amount: reader.u64("lp_fee")?,
        protocol_fee_basis_points: reader.u64("protocol_fee_basis_points")?,
        protocol_fee_amount: reader.u64("protocol_fee")?,
        quote_amount_out_without_lp_fee: reader.u64("quote_amount_out_without_lp_fee")?,
        user_quote_amount_out: reader.u64("user_quote_amount_out")?,
        pool: reader.pubkey("pool")?,
        user: reader.pubkey("user")?,
        user_base_token_account: reader.pubkey("user_base_token_account")?,
        user_quote_token_account: reader.pubkey("user_quote_token_account")?,
        protocol_fee_recipient: reader.pubkey("protocol_fee_recipient")?,
        protocol_fee_recipient_token_account: reader
            .pubkey("protocol_fee_recipient_token_account")?,
        coin_creator: reader.pubkey("coin_creator")?,
        coin_creator_fee_basis_points: reader.u64("coin_creator_fee_basis_points")?,
        coin_creator_fee_amount: reader.u64("coin_creator_fee")?,
        cashback_fee_basis_points: reader.u64("cashback_fee_basis_points")?,
        cashback_amount: reader.u64("cashback")?,
        buyback_fee_basis_points: reader.u64("buyback_fee_basis_points")?,
        buyback_fee_amount: reader.u64("buyback_fee")?,
        virtual_quote_reserves: reader.i128("virtual_quote_reserves")?,
        can_boost: reader.boolean("can_boost")?,
        base_supply: reader.u64("base_supply")?,
    })
}

fn decode_pump_swap_deposit(
    reader: &mut BorshReader<'_>,
) -> Result<PumpSwapDepositEvent, DecodeError> {
    Ok(PumpSwapDepositEvent {
        timestamp: reader.i64("timestamp")?,
        lp_token_amount_out: reader.u64("lp_token_amount_out")?,
        max_base_amount_in: reader.u64("max_base_amount_in")?,
        max_quote_amount_in: reader.u64("max_quote_amount_in")?,
        user_base_token_reserves: reader.u64("user_base_token_reserves")?,
        user_quote_token_reserves: reader.u64("user_quote_token_reserves")?,
        pool_base_token_reserves: reader.u64("pool_base_token_reserves")?,
        pool_quote_token_reserves: reader.u64("pool_quote_token_reserves")?,
        base_amount_in: reader.u64("base_amount_in")?,
        quote_amount_in: reader.u64("quote_amount_in")?,
        lp_mint_supply: reader.u64("lp_mint_supply")?,
        pool: reader.pubkey("pool")?,
        user: reader.pubkey("user")?,
        user_base_token_account: reader.pubkey("user_base_token_account")?,
        user_quote_token_account: reader.pubkey("user_quote_token_account")?,
        user_pool_token_account: reader.pubkey("user_pool_token_account")?,
    })
}

fn decode_pump_swap_withdraw(
    reader: &mut BorshReader<'_>,
) -> Result<PumpSwapWithdrawEvent, DecodeError> {
    Ok(PumpSwapWithdrawEvent {
        timestamp: reader.i64("timestamp")?,
        lp_token_amount_in: reader.u64("lp_token_amount_in")?,
        min_base_amount_out: reader.u64("min_base_amount_out")?,
        min_quote_amount_out: reader.u64("min_quote_amount_out")?,
        user_base_token_reserves: reader.u64("user_base_token_reserves")?,
        user_quote_token_reserves: reader.u64("user_quote_token_reserves")?,
        pool_base_token_reserves: reader.u64("pool_base_token_reserves")?,
        pool_quote_token_reserves: reader.u64("pool_quote_token_reserves")?,
        base_amount_out: reader.u64("base_amount_out")?,
        quote_amount_out: reader.u64("quote_amount_out")?,
        lp_mint_supply: reader.u64("lp_mint_supply")?,
        pool: reader.pubkey("pool")?,
        user: reader.pubkey("user")?,
        user_base_token_account: reader.pubkey("user_base_token_account")?,
        user_quote_token_account: reader.pubkey("user_quote_token_account")?,
        user_pool_token_account: reader.pubkey("user_pool_token_account")?,
    })
}

struct BorshReader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> BorshReader<'a> {
    const fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn finish(self) -> Result<(), DecodeError> {
        let remaining = self.data.len().saturating_sub(self.position);
        if remaining == 0 {
            Ok(())
        } else {
            Err(DecodeError::TrailingBytes(remaining))
        }
    }

    fn take(&mut self, field: &'static str, length: usize) -> Result<&'a [u8], DecodeError> {
        let remaining = self.data.len().saturating_sub(self.position);
        if remaining < length {
            return Err(DecodeError::Truncated {
                field,
                needed: length,
                remaining,
            });
        }
        let start = self.position;
        self.position += length;
        Ok(&self.data[start..self.position])
    }

    fn u8(&mut self, field: &'static str) -> Result<u8, DecodeError> {
        Ok(self.take(field, 1)?[0])
    }

    fn u16(&mut self, field: &'static str) -> Result<u16, DecodeError> {
        let bytes: [u8; 2] = self
            .take(field, 2)?
            .try_into()
            .expect("slice length is checked");
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self, field: &'static str) -> Result<u32, DecodeError> {
        let bytes: [u8; 4] = self
            .take(field, 4)?
            .try_into()
            .expect("slice length is checked");
        Ok(u32::from_le_bytes(bytes))
    }

    fn u64(&mut self, field: &'static str) -> Result<u64, DecodeError> {
        let bytes: [u8; 8] = self
            .take(field, 8)?
            .try_into()
            .expect("slice length is checked");
        Ok(u64::from_le_bytes(bytes))
    }

    fn i64(&mut self, field: &'static str) -> Result<i64, DecodeError> {
        let bytes: [u8; 8] = self
            .take(field, 8)?
            .try_into()
            .expect("slice length is checked");
        Ok(i64::from_le_bytes(bytes))
    }

    fn i128(&mut self, field: &'static str) -> Result<i128, DecodeError> {
        let bytes: [u8; 16] = self
            .take(field, 16)?
            .try_into()
            .expect("slice length is checked");
        Ok(i128::from_le_bytes(bytes))
    }

    fn boolean(&mut self, field: &'static str) -> Result<bool, DecodeError> {
        match self.u8(field)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(DecodeError::InvalidBoolean { field, value }),
        }
    }

    fn pubkey(&mut self, field: &'static str) -> Result<String, DecodeError> {
        Ok(encode_base58(self.take(field, 32)?))
    }

    fn string(&mut self, field: &'static str, limit: usize) -> Result<String, DecodeError> {
        let length = usize::try_from(self.u32(field)?).map_err(|_| DecodeError::LimitExceeded {
            field,
            limit,
            actual: usize::MAX,
        })?;
        if length > limit {
            return Err(DecodeError::LimitExceeded {
                field,
                limit,
                actual: length,
            });
        }
        let bytes = self.take(field, length)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| DecodeError::InvalidUtf8 { field })
    }

    fn shareholders(&mut self) -> Result<Vec<PumpShareholder>, DecodeError> {
        let count =
            usize::try_from(self.u32("shareholders")?).map_err(|_| DecodeError::LimitExceeded {
                field: "shareholders",
                limit: MAX_SHAREHOLDERS,
                actual: usize::MAX,
            })?;
        if count > MAX_SHAREHOLDERS {
            return Err(DecodeError::LimitExceeded {
                field: "shareholders",
                limit: MAX_SHAREHOLDERS,
                actual: count,
            });
        }
        let mut shareholders = Vec::with_capacity(count);
        for _ in 0..count {
            shareholders.push(PumpShareholder {
                address: self.pubkey("shareholder.address")?,
                share_basis_points: self.u16("shareholder.share_bps")?,
            });
        }
        Ok(shareholders)
    }
}

fn encode_base58(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    let leading_zeroes = bytes.iter().take_while(|byte| **byte == 0).count();
    let mut digits = vec![0_u8];
    for byte in bytes {
        let mut carry = u32::from(*byte);
        for digit in &mut digits {
            let value = u32::from(*digit) * 256 + carry;
            *digit = u8::try_from(value % 58).expect("base58 digit fits in u8");
            carry = value / 58;
        }
        while carry > 0 {
            digits.push(u8::try_from(carry % 58).expect("base58 digit fits in u8"));
            carry /= 58;
        }
    }

    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
    }

    let mut encoded = String::with_capacity(leading_zeroes + digits.len());
    encoded.extend(std::iter::repeat_n('1', leading_zeroes));
    if leading_zeroes != bytes.len() {
        for digit in digits.iter().rev() {
            encoded.push(char::from(ALPHABET[usize::from(*digit)]));
        }
    }
    encoded
}

#[cfg(test)]
mod unit_tests {
    use super::{DecodeError, decode_program_data_bytes, encode_base58};

    #[test]
    fn base58_encodes_zero_pubkey_and_known_system_program_bytes() {
        assert_eq!(encode_base58(&[0; 32]), "11111111111111111111111111111111");

        let mut bytes = [0_u8; 32];
        bytes[31] = 1;
        assert_eq!(encode_base58(&bytes), "11111111111111111111111111111112");
    }

    #[test]
    fn program_data_evidence_is_the_exact_decoded_payload() {
        assert_eq!(
            decode_program_data_bytes("Program data: AQID").expect("valid base64"),
            [1, 2, 3]
        );
        assert!(matches!(
            decode_program_data_bytes("not program data"),
            Err(DecodeError::InvalidProgramDataLog)
        ));
    }
}
