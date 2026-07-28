use serde::{Deserialize, Serialize};
use soldisco_domain::ChainCoordinate;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PumpProgram {
    Pump,
    PumpSwap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PumpEventKind {
    Create,
    Trade,
    Complete,
    CompletePumpAmmMigration,
    CreatePool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TradeDirection {
    Buy,
    Sell,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct DecodedPumpEvent {
    pub program: PumpProgram,
    pub coordinate: ChainCoordinate,
    pub event: PumpEvent,
}

impl DecodedPumpEvent {
    #[must_use]
    pub const fn event_kind(&self) -> PumpEventKind {
        self.event.kind()
    }

    #[must_use]
    pub const fn source_event_time_unix_seconds(&self) -> i64 {
        self.event.timestamp()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "event_type",
    content = "event",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum PumpEvent {
    Create(PumpCreateEvent),
    Trade(PumpTradeEvent),
    Complete(PumpCompleteEvent),
    CompletePumpAmmMigration(PumpMigrationEvent),
    PumpSwapCreatePool(PumpSwapCreatePoolEvent),
    PumpSwapBuy(PumpSwapBuyEvent),
    PumpSwapSell(PumpSwapSellEvent),
}

impl PumpEvent {
    #[must_use]
    pub const fn kind(&self) -> PumpEventKind {
        match self {
            Self::Create(_) => PumpEventKind::Create,
            Self::Trade(_) | Self::PumpSwapBuy(_) | Self::PumpSwapSell(_) => PumpEventKind::Trade,
            Self::Complete(_) => PumpEventKind::Complete,
            Self::CompletePumpAmmMigration(_) => PumpEventKind::CompletePumpAmmMigration,
            Self::PumpSwapCreatePool(_) => PumpEventKind::CreatePool,
        }
    }

    #[must_use]
    pub const fn timestamp(&self) -> i64 {
        match self {
            Self::Create(event) => event.timestamp,
            Self::Trade(event) => event.timestamp,
            Self::Complete(event) => event.timestamp,
            Self::CompletePumpAmmMigration(event) => event.timestamp,
            Self::PumpSwapCreatePool(event) => event.timestamp,
            Self::PumpSwapBuy(event) => event.timestamp,
            Self::PumpSwapSell(event) => event.timestamp,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PumpShareholder {
    pub address: String,
    pub share_basis_points: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PumpCreateEvent {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub mint: String,
    pub bonding_curve: String,
    pub user: String,
    pub creator: String,
    pub timestamp: i64,
    pub virtual_token_reserves: u64,
    /// Legacy SOL-specific reserve retained by the current public IDL.
    pub virtual_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub token_total_supply: u64,
    pub token_program: String,
    pub is_mayhem_mode: bool,
    pub is_cashback_enabled: bool,
    pub quote_mint: String,
    pub virtual_quote_reserves: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PumpTradeEvent {
    pub mint: String,
    /// Legacy SOL-specific amount retained by the current public IDL. Consumers
    /// should use `quote_amount` with `quote_mint` for market volume.
    pub legacy_sol_amount: u64,
    pub token_amount: u64,
    pub direction: TradeDirection,
    pub user: String,
    pub timestamp: i64,
    /// Legacy SOL-specific reserve retained by the current public IDL.
    pub virtual_sol_reserves: u64,
    pub virtual_token_reserves: u64,
    /// Legacy SOL-specific reserve retained by the current public IDL.
    pub real_sol_reserves: u64,
    pub real_token_reserves: u64,
    pub fee_recipient: String,
    pub fee_basis_points: u64,
    pub fee_amount: u64,
    pub creator: String,
    pub creator_fee_basis_points: u64,
    pub creator_fee_amount: u64,
    pub track_volume: bool,
    pub total_unclaimed_tokens: u64,
    pub total_claimed_tokens: u64,
    pub current_sol_volume: u64,
    pub last_update_timestamp: i64,
    pub instruction_name: String,
    pub mayhem_mode: bool,
    pub cashback_fee_basis_points: u64,
    pub cashback_amount: u64,
    pub buyback_fee_basis_points: u64,
    pub buyback_fee_amount: u64,
    pub shareholders: Vec<PumpShareholder>,
    pub quote_mint: String,
    pub quote_amount: u64,
    pub virtual_quote_reserves: u64,
    pub real_quote_reserves: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PumpCompleteEvent {
    pub user: String,
    pub mint: String,
    pub bonding_curve: String,
    pub timestamp: i64,
    pub quote_mint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PumpMigrationEvent {
    pub user: String,
    pub mint: String,
    pub mint_amount: u64,
    /// Legacy SOL-specific amount retained by the current public IDL.
    pub legacy_sol_amount: u64,
    pub pool_migration_fee: u64,
    pub bonding_curve: String,
    pub timestamp: i64,
    pub pool: String,
    pub quote_mint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PumpSwapCreatePoolEvent {
    pub timestamp: i64,
    pub index: u16,
    pub creator: String,
    pub base_mint: String,
    pub quote_mint: String,
    pub base_mint_decimals: u8,
    pub quote_mint_decimals: u8,
    pub base_amount_in: u64,
    pub quote_amount_in: u64,
    pub pool_base_amount: u64,
    pub pool_quote_amount: u64,
    pub minimum_liquidity: u64,
    pub initial_liquidity: u64,
    pub lp_token_amount_out: u64,
    pub pool_bump: u8,
    pub pool: String,
    pub lp_mint: String,
    pub user_base_token_account: String,
    pub user_quote_token_account: String,
    pub coin_creator: String,
    pub is_mayhem_mode: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PumpSwapBuyEvent {
    pub timestamp: i64,
    pub base_amount_out: u64,
    pub max_quote_amount_in: u64,
    pub user_base_token_reserves: u64,
    pub user_quote_token_reserves: u64,
    pub pool_base_token_reserves: u64,
    pub pool_quote_token_reserves: u64,
    pub quote_amount_in: u64,
    pub lp_fee_basis_points: u64,
    pub lp_fee_amount: u64,
    pub protocol_fee_basis_points: u64,
    pub protocol_fee_amount: u64,
    pub quote_amount_in_with_lp_fee: u64,
    pub user_quote_amount_in: u64,
    pub pool: String,
    pub user: String,
    pub user_base_token_account: String,
    pub user_quote_token_account: String,
    pub protocol_fee_recipient: String,
    pub protocol_fee_recipient_token_account: String,
    pub coin_creator: String,
    pub coin_creator_fee_basis_points: u64,
    pub coin_creator_fee_amount: u64,
    pub track_volume: bool,
    pub total_unclaimed_tokens: u64,
    pub total_claimed_tokens: u64,
    pub current_sol_volume: u64,
    pub last_update_timestamp: i64,
    pub min_base_amount_out: u64,
    pub instruction_name: String,
    pub cashback_fee_basis_points: u64,
    pub cashback_amount: u64,
    pub buyback_fee_basis_points: u64,
    pub buyback_fee_amount: u64,
    pub virtual_quote_reserves: i128,
    pub can_boost: bool,
    pub base_supply: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PumpSwapSellEvent {
    pub timestamp: i64,
    pub base_amount_in: u64,
    pub min_quote_amount_out: u64,
    pub user_base_token_reserves: u64,
    pub user_quote_token_reserves: u64,
    pub pool_base_token_reserves: u64,
    pub pool_quote_token_reserves: u64,
    pub quote_amount_out: u64,
    pub lp_fee_basis_points: u64,
    pub lp_fee_amount: u64,
    pub protocol_fee_basis_points: u64,
    pub protocol_fee_amount: u64,
    pub quote_amount_out_without_lp_fee: u64,
    pub user_quote_amount_out: u64,
    pub pool: String,
    pub user: String,
    pub user_base_token_account: String,
    pub user_quote_token_account: String,
    pub protocol_fee_recipient: String,
    pub protocol_fee_recipient_token_account: String,
    pub coin_creator: String,
    pub coin_creator_fee_basis_points: u64,
    pub coin_creator_fee_amount: u64,
    pub cashback_fee_basis_points: u64,
    pub cashback_amount: u64,
    pub buyback_fee_basis_points: u64,
    pub buyback_fee_amount: u64,
    pub virtual_quote_reserves: i128,
    pub can_boost: bool,
    pub base_supply: u64,
}
