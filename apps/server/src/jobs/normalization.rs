use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use soldisco_domain::{
    Commitment, MarketIdentity, Network, NormalizedObservation, ObservationKey, ObservationPayload,
    TradeSide, Venue,
};
use soldisco_source_pump::{
    DECODER_SCHEMA_VERSION, DECODER_VERSION, DecodedPumpEvent, PumpEvent, TradeDirection,
};
use thiserror::Error;

use super::pump_swap_pair::{PumpSwapPair, PumpSwapSourceOrientation, normalize_pump_swap_pair};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum MarketLookupKey {
    PumpMint(String),
    PumpSwapPool(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RegisteredMarket {
    identity: MarketIdentity,
    pump_swap_source_orientation: Option<PumpSwapSourceOrientation>,
}

#[derive(Clone, Default)]
pub struct MarketRegistry {
    markets: Arc<RwLock<HashMap<MarketLookupKey, RegisteredMarket>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedPumpEvent {
    pub observation: NormalizedObservation,
    pub discovered_market: Option<MarketIdentity>,
}

#[derive(Debug, Error)]
pub enum NormalizationError {
    #[error("market registry lock is unavailable")]
    RegistryUnavailable,
    #[error("no Pump bonding-curve market is known for mint {0}")]
    PumpMarketUnresolved(String),
    #[error("no PumpSwap market is known for pool {0}")]
    PumpSwapMarketUnresolved(String),
    #[error(
        "PumpSwap pair {base_mint}/{quote_mint} does not contain exactly one supported quote asset"
    )]
    UnsupportedPumpSwapPair {
        base_mint: String,
        quote_mint: String,
    },
    #[error("PumpSwap markets must be registered from their source pool orientation")]
    PumpSwapSourceOrientationRequired,
    #[error("Pump trade quote mint does not match the registered market for mint {mint}")]
    QuoteMintMismatch { mint: String },
    #[error("trade base and quote amounts must both be non-zero")]
    ZeroTradeAmount,
    #[error("decoded Pump event could not be represented as source detail JSON: {0}")]
    SourceDetails(#[from] serde_json::Error),
}

impl MarketRegistry {
    pub fn register(&self, market: MarketIdentity) -> Result<(), NormalizationError> {
        let key = match market.venue {
            Venue::PumpBondingCurve => MarketLookupKey::PumpMint(market.mint.clone()),
            Venue::PumpSwap => {
                return Err(NormalizationError::PumpSwapSourceOrientationRequired);
            }
            Venue::RaydiumCpmm | Venue::RaydiumClmm | Venue::RaydiumAmmV4 => return Ok(()),
        };
        self.markets
            .write()
            .map_err(|_| NormalizationError::RegistryUnavailable)?
            .insert(
                key,
                RegisteredMarket {
                    identity: market,
                    pump_swap_source_orientation: None,
                },
            );
        Ok(())
    }

    pub fn register_pump_swap_pool(
        &self,
        network: Network,
        pool: &str,
        source_base_mint: &str,
        source_quote_mint: &str,
    ) -> Result<Option<MarketIdentity>, NormalizationError> {
        let Some(pair) = normalize_pump_swap_pair(network, source_base_mint, source_quote_mint)
        else {
            return Ok(None);
        };
        let market = pump_swap_market_identity(network, pool, &pair);
        self.markets
            .write()
            .map_err(|_| NormalizationError::RegistryUnavailable)?
            .insert(
                MarketLookupKey::PumpSwapPool(pool.to_owned()),
                RegisteredMarket {
                    identity: market.clone(),
                    pump_swap_source_orientation: Some(pair.source_orientation),
                },
            );
        Ok(Some(market))
    }

    pub fn register_all(
        &self,
        markets: impl IntoIterator<Item = MarketIdentity>,
    ) -> Result<(), NormalizationError> {
        for market in markets {
            self.register(market)?;
        }
        Ok(())
    }

    pub fn retire_pump_market(&self, mint: &str) -> Result<(), NormalizationError> {
        self.markets
            .write()
            .map_err(|_| NormalizationError::RegistryUnavailable)?
            .remove(&MarketLookupKey::PumpMint(mint.to_owned()));
        Ok(())
    }

    fn pump_market(&self, mint: &str) -> Result<MarketIdentity, NormalizationError> {
        self.markets
            .read()
            .map_err(|_| NormalizationError::RegistryUnavailable)?
            .get(&MarketLookupKey::PumpMint(mint.to_owned()))
            .map(|market| market.identity.clone())
            .ok_or_else(|| NormalizationError::PumpMarketUnresolved(mint.to_owned()))
    }

    fn pump_swap_market(&self, pool: &str) -> Result<RegisteredMarket, NormalizationError> {
        self.markets
            .read()
            .map_err(|_| NormalizationError::RegistryUnavailable)?
            .get(&MarketLookupKey::PumpSwapPool(pool.to_owned()))
            .cloned()
            .ok_or_else(|| NormalizationError::PumpSwapMarketUnresolved(pool.to_owned()))
    }
}

pub fn normalize_pump_event(
    decoded: DecodedPumpEvent,
    network: Network,
    commitment: Commitment,
    received_time_unix_ms: i64,
    raw_evidence: &[u8],
    markets: &MarketRegistry,
) -> Result<NormalizedPumpEvent, NormalizationError> {
    let source_program = decoded.program.source_program();
    let source_details = serde_json::to_value(&decoded.event)?;
    let source_event_time_unix_ms = decoded
        .source_event_time_unix_seconds()
        .checked_mul(1_000)
        .filter(|timestamp| *timestamp >= 0);

    let (event_kind, market, payload, discovered_market) = match decoded.event {
        PumpEvent::Create(event) => {
            let market = MarketIdentity {
                network,
                mint: event.mint,
                venue: Venue::PumpBondingCurve,
                market_address: event.bonding_curve,
                quote_mint: Some(event.quote_mint),
            };
            (
                "CREATE",
                market.clone(),
                ObservationPayload::TokenCreated {
                    name: event.name,
                    symbol: event.symbol,
                    uri: event.uri,
                    creator: event.creator,
                    user: event.user,
                },
                Some(market),
            )
        }
        PumpEvent::Trade(event) => {
            require_non_zero_trade_amounts(event.token_amount, event.quote_amount)?;
            let market = markets.pump_market(&event.mint)?;
            if market.quote_mint.as_deref() != Some(event.quote_mint.as_str()) {
                return Err(NormalizationError::QuoteMintMismatch { mint: event.mint });
            }
            (
                "TRADE",
                market,
                ObservationPayload::Trade {
                    side: trade_side(event.direction),
                    wallet: event.user,
                    base_amount_units: event.token_amount,
                    quote_amount_units: event.quote_amount,
                    base_reserve_units: Some(event.virtual_token_reserves),
                    quote_reserve_units: Some(event.virtual_quote_reserves),
                },
                None,
            )
        }
        PumpEvent::Complete(event) => (
            "COMPLETE",
            MarketIdentity {
                network,
                mint: event.mint,
                venue: Venue::PumpBondingCurve,
                market_address: event.bonding_curve,
                quote_mint: Some(event.quote_mint),
            },
            ObservationPayload::MarketCompleted { user: event.user },
            None,
        ),
        PumpEvent::CompletePumpAmmMigration(event) => (
            "COMPLETE_PUMP_AMM_MIGRATION",
            MarketIdentity {
                network,
                mint: event.mint,
                venue: Venue::PumpBondingCurve,
                market_address: event.bonding_curve,
                quote_mint: Some(event.quote_mint),
            },
            ObservationPayload::MarketMigrated {
                user: event.user,
                destination_market: event.pool,
                base_amount_units: event.mint_amount,
                legacy_sol_amount_units: event.legacy_sol_amount,
            },
            None,
        ),
        PumpEvent::PumpSwapCreatePool(event) => {
            let pair = normalize_pump_swap_pair(network, &event.base_mint, &event.quote_mint)
                .ok_or_else(|| NormalizationError::UnsupportedPumpSwapPair {
                    base_mint: event.base_mint.clone(),
                    quote_mint: event.quote_mint.clone(),
                })?;
            let market = pump_swap_market_identity(network, &event.pool, &pair);
            let (base_amount_units, quote_amount_units) = orient_source_amounts(
                pair.source_orientation,
                event.base_amount_in,
                event.quote_amount_in,
            );
            (
                "CREATE_POOL",
                market.clone(),
                ObservationPayload::MarketCreated {
                    creator: event.creator,
                    base_amount_units,
                    quote_amount_units,
                },
                Some(market),
            )
        }
        PumpEvent::PumpSwapBuy(event) => {
            require_non_zero_trade_amounts(event.base_amount_out, event.quote_amount_in)?;
            let registered = markets.pump_swap_market(&event.pool)?;
            let source_orientation = registered
                .pump_swap_source_orientation
                .ok_or(NormalizationError::PumpSwapSourceOrientationRequired)?;
            let (side, base_amount_units, quote_amount_units) = match source_orientation {
                PumpSwapSourceOrientation::BaseIsToken => {
                    (TradeSide::Buy, event.base_amount_out, event.quote_amount_in)
                }
                PumpSwapSourceOrientation::QuoteIsToken => (
                    TradeSide::Sell,
                    event.quote_amount_in,
                    event.base_amount_out,
                ),
            };
            let (base_reserve_units, quote_reserve_units) = orient_source_amounts(
                source_orientation,
                event.pool_base_token_reserves,
                event.pool_quote_token_reserves,
            );
            let event_kind = match side {
                TradeSide::Buy => "BUY",
                TradeSide::Sell => "SELL",
            };
            (
                event_kind,
                registered.identity,
                ObservationPayload::Trade {
                    side,
                    wallet: event.user,
                    base_amount_units,
                    quote_amount_units,
                    base_reserve_units: Some(base_reserve_units),
                    quote_reserve_units: Some(quote_reserve_units),
                },
                None,
            )
        }
        PumpEvent::PumpSwapSell(event) => {
            require_non_zero_trade_amounts(event.base_amount_in, event.quote_amount_out)?;
            let registered = markets.pump_swap_market(&event.pool)?;
            let source_orientation = registered
                .pump_swap_source_orientation
                .ok_or(NormalizationError::PumpSwapSourceOrientationRequired)?;
            let (side, base_amount_units, quote_amount_units) = match source_orientation {
                PumpSwapSourceOrientation::BaseIsToken => (
                    TradeSide::Sell,
                    event.base_amount_in,
                    event.quote_amount_out,
                ),
                PumpSwapSourceOrientation::QuoteIsToken => {
                    (TradeSide::Buy, event.quote_amount_out, event.base_amount_in)
                }
            };
            let (base_reserve_units, quote_reserve_units) = orient_source_amounts(
                source_orientation,
                event.pool_base_token_reserves,
                event.pool_quote_token_reserves,
            );
            let event_kind = match side {
                TradeSide::Buy => "BUY",
                TradeSide::Sell => "SELL",
            };
            (
                event_kind,
                registered.identity,
                ObservationPayload::Trade {
                    side,
                    wallet: event.user,
                    base_amount_units,
                    quote_amount_units,
                    base_reserve_units: Some(base_reserve_units),
                    quote_reserve_units: Some(quote_reserve_units),
                },
                None,
            )
        }
        PumpEvent::PumpSwapDeposit(event) => {
            let registered = markets.pump_swap_market(&event.pool)?;
            let source_orientation = registered
                .pump_swap_source_orientation
                .ok_or(NormalizationError::PumpSwapSourceOrientationRequired)?;
            let (base_amount_units, quote_amount_units) = orient_source_amounts(
                source_orientation,
                event.base_amount_in,
                event.quote_amount_in,
            );
            let (base_reserve_units, quote_reserve_units) = orient_source_amounts(
                source_orientation,
                event.pool_base_token_reserves,
                event.pool_quote_token_reserves,
            );
            (
                "LIQUIDITY_DEPOSIT",
                registered.identity,
                ObservationPayload::LiquidityDeposited {
                    provider: event.user,
                    base_amount_units,
                    quote_amount_units,
                    base_reserve_units,
                    quote_reserve_units,
                    lp_token_amount_units: event.lp_token_amount_out,
                    lp_token_supply_units: event.lp_mint_supply,
                },
                None,
            )
        }
        PumpEvent::PumpSwapWithdraw(event) => {
            let registered = markets.pump_swap_market(&event.pool)?;
            let source_orientation = registered
                .pump_swap_source_orientation
                .ok_or(NormalizationError::PumpSwapSourceOrientationRequired)?;
            let (base_amount_units, quote_amount_units) = orient_source_amounts(
                source_orientation,
                event.base_amount_out,
                event.quote_amount_out,
            );
            let (base_reserve_units, quote_reserve_units) = orient_source_amounts(
                source_orientation,
                event.pool_base_token_reserves,
                event.pool_quote_token_reserves,
            );
            (
                "LIQUIDITY_WITHDRAW",
                registered.identity,
                ObservationPayload::LiquidityWithdrawn {
                    provider: event.user,
                    base_amount_units,
                    quote_amount_units,
                    base_reserve_units,
                    quote_reserve_units,
                    lp_token_amount_units: event.lp_token_amount_in,
                    lp_token_supply_units: event.lp_mint_supply,
                },
                None,
            )
        }
    };

    let observation = NormalizedObservation {
        key: ObservationKey {
            network,
            program: source_program,
            coordinate: decoded.coordinate,
        },
        commitment,
        schema_version: DECODER_SCHEMA_VERSION,
        decoder_version: DECODER_VERSION.to_owned(),
        event_kind: event_kind.to_owned(),
        market,
        source_event_time_unix_ms,
        received_time_unix_ms,
        raw_evidence_hash: evidence_hash(raw_evidence),
        source_evidence_base64: STANDARD.encode(raw_evidence),
        source_details,
        payload,
    };

    Ok(NormalizedPumpEvent {
        observation,
        discovered_market,
    })
}

fn pump_swap_market_identity(network: Network, pool: &str, pair: &PumpSwapPair) -> MarketIdentity {
    MarketIdentity {
        network,
        mint: pair.token_mint.clone(),
        venue: Venue::PumpSwap,
        market_address: pool.to_owned(),
        quote_mint: Some(pair.quote_mint.clone()),
    }
}

fn orient_source_amounts(
    source_orientation: PumpSwapSourceOrientation,
    source_base_units: u64,
    source_quote_units: u64,
) -> (u64, u64) {
    match source_orientation {
        PumpSwapSourceOrientation::BaseIsToken => (source_base_units, source_quote_units),
        PumpSwapSourceOrientation::QuoteIsToken => (source_quote_units, source_base_units),
    }
}

fn trade_side(direction: TradeDirection) -> TradeSide {
    match direction {
        TradeDirection::Buy => TradeSide::Buy,
        TradeDirection::Sell => TradeSide::Sell,
    }
}

fn require_non_zero_trade_amounts(
    base_amount_units: u64,
    quote_amount_units: u64,
) -> Result<(), NormalizationError> {
    if base_amount_units == 0 || quote_amount_units == 0 {
        return Err(NormalizationError::ZeroTradeAmount);
    }
    Ok(())
}

fn evidence_hash(raw_evidence: &[u8]) -> String {
    let digest = Sha256::digest(raw_evidence);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use soldisco_discovery_engine::WRAPPED_SOL_MINT;
    use soldisco_domain::{
        ChainCoordinate, Commitment, Network, ObservationPayload, TradeSide, Venue,
    };
    use soldisco_source_pump::{
        DecodedPumpEvent, PumpCreateEvent, PumpEvent, PumpProgram, PumpSwapBuyEvent,
        PumpSwapCreatePoolEvent, PumpSwapDepositEvent, PumpSwapSellEvent, PumpSwapWithdrawEvent,
        PumpTradeEvent, TradeDirection,
    };

    use super::{MarketRegistry, NormalizationError, normalize_pump_event};

    fn coordinate(event_index: u16) -> ChainCoordinate {
        ChainCoordinate {
            slot: 42,
            transaction_index: None,
            signature: "signature".to_owned(),
            instruction_index: 1,
            event_index,
        }
    }

    fn pump_trade_event(quote_amount: u64) -> PumpTradeEvent {
        PumpTradeEvent {
            mint: "mint".to_owned(),
            legacy_sol_amount: quote_amount,
            token_amount: 20,
            direction: TradeDirection::Sell,
            user: "wallet".to_owned(),
            timestamp: 100,
            virtual_sol_reserves: 1,
            virtual_token_reserves: 2,
            real_sol_reserves: 3,
            real_token_reserves: 4,
            fee_recipient: "fee".to_owned(),
            fee_basis_points: 0,
            fee_amount: 0,
            creator: "creator".to_owned(),
            creator_fee_basis_points: 0,
            creator_fee_amount: 0,
            track_volume: true,
            total_unclaimed_tokens: 0,
            total_claimed_tokens: 0,
            current_sol_volume: 0,
            last_update_timestamp: 100,
            instruction_name: "sell".to_owned(),
            mayhem_mode: false,
            cashback_fee_basis_points: 0,
            cashback_amount: 0,
            buyback_fee_basis_points: 0,
            buyback_fee_amount: 0,
            shareholders: Vec::new(),
            quote_mint: "quote".to_owned(),
            quote_amount,
            virtual_quote_reserves: 1,
            real_quote_reserves: 2,
        }
    }

    fn pump_swap_create_event(base_mint: &str, quote_mint: &str) -> PumpSwapCreatePoolEvent {
        PumpSwapCreatePoolEvent {
            timestamp: 100,
            index: 1,
            creator: "creator".to_owned(),
            base_mint: base_mint.to_owned(),
            quote_mint: quote_mint.to_owned(),
            base_mint_decimals: 9,
            quote_mint_decimals: 6,
            base_amount_in: 11,
            quote_amount_in: 22,
            pool_base_amount: 101,
            pool_quote_amount: 202,
            minimum_liquidity: 1,
            initial_liquidity: 2,
            lp_token_amount_out: 3,
            pool_bump: 254,
            pool: "pool".to_owned(),
            lp_mint: "lp-mint".to_owned(),
            user_base_token_account: "base-account".to_owned(),
            user_quote_token_account: "quote-account".to_owned(),
            coin_creator: "coin-creator".to_owned(),
            is_mayhem_mode: false,
        }
    }

    fn pump_swap_buy_event() -> PumpSwapBuyEvent {
        PumpSwapBuyEvent {
            timestamp: 101,
            base_amount_out: 11,
            max_quote_amount_in: 23,
            user_base_token_reserves: 100,
            user_quote_token_reserves: 200,
            pool_base_token_reserves: 101,
            pool_quote_token_reserves: 202,
            quote_amount_in: 22,
            lp_fee_basis_points: 1,
            lp_fee_amount: 2,
            protocol_fee_basis_points: 3,
            protocol_fee_amount: 4,
            quote_amount_in_with_lp_fee: 24,
            user_quote_amount_in: 25,
            pool: "pool".to_owned(),
            user: "wallet".to_owned(),
            user_base_token_account: "base-account".to_owned(),
            user_quote_token_account: "quote-account".to_owned(),
            protocol_fee_recipient: "fee-recipient".to_owned(),
            protocol_fee_recipient_token_account: "fee-account".to_owned(),
            coin_creator: "coin-creator".to_owned(),
            coin_creator_fee_basis_points: 5,
            coin_creator_fee_amount: 6,
            track_volume: true,
            total_unclaimed_tokens: 7,
            total_claimed_tokens: 8,
            current_sol_volume: 9,
            last_update_timestamp: 101,
            min_base_amount_out: 10,
            instruction_name: "buy".to_owned(),
            cashback_fee_basis_points: 11,
            cashback_amount: 12,
            buyback_fee_basis_points: 13,
            buyback_fee_amount: 14,
            virtual_quote_reserves: 15,
            can_boost: true,
            base_supply: 16,
        }
    }

    fn pump_swap_sell_event() -> PumpSwapSellEvent {
        PumpSwapSellEvent {
            timestamp: 102,
            base_amount_in: 33,
            min_quote_amount_out: 43,
            user_base_token_reserves: 300,
            user_quote_token_reserves: 400,
            pool_base_token_reserves: 303,
            pool_quote_token_reserves: 404,
            quote_amount_out: 44,
            lp_fee_basis_points: 1,
            lp_fee_amount: 2,
            protocol_fee_basis_points: 3,
            protocol_fee_amount: 4,
            quote_amount_out_without_lp_fee: 45,
            user_quote_amount_out: 46,
            pool: "pool".to_owned(),
            user: "wallet".to_owned(),
            user_base_token_account: "base-account".to_owned(),
            user_quote_token_account: "quote-account".to_owned(),
            protocol_fee_recipient: "fee-recipient".to_owned(),
            protocol_fee_recipient_token_account: "fee-account".to_owned(),
            coin_creator: "coin-creator".to_owned(),
            coin_creator_fee_basis_points: 5,
            coin_creator_fee_amount: 6,
            cashback_fee_basis_points: 7,
            cashback_amount: 8,
            buyback_fee_basis_points: 9,
            buyback_fee_amount: 10,
            virtual_quote_reserves: 11,
            can_boost: true,
            base_supply: 12,
        }
    }

    fn pump_swap_deposit_event() -> PumpSwapDepositEvent {
        PumpSwapDepositEvent {
            timestamp: 103,
            lp_token_amount_out: 3,
            max_base_amount_in: 12,
            max_quote_amount_in: 23,
            user_base_token_reserves: 100,
            user_quote_token_reserves: 200,
            pool_base_token_reserves: 101,
            pool_quote_token_reserves: 202,
            base_amount_in: 11,
            quote_amount_in: 22,
            lp_mint_supply: 1_000,
            pool: "pool".to_owned(),
            user: "liquidity-provider".to_owned(),
            user_base_token_account: "base-account".to_owned(),
            user_quote_token_account: "quote-account".to_owned(),
            user_pool_token_account: "pool-account".to_owned(),
        }
    }

    fn pump_swap_withdraw_event() -> PumpSwapWithdrawEvent {
        PumpSwapWithdrawEvent {
            timestamp: 104,
            lp_token_amount_in: 5,
            min_base_amount_out: 32,
            min_quote_amount_out: 43,
            user_base_token_reserves: 300,
            user_quote_token_reserves: 400,
            pool_base_token_reserves: 303,
            pool_quote_token_reserves: 404,
            base_amount_out: 33,
            quote_amount_out: 44,
            lp_mint_supply: 900,
            pool: "pool".to_owned(),
            user: "liquidity-provider".to_owned(),
            user_base_token_account: "base-account".to_owned(),
            user_quote_token_account: "quote-account".to_owned(),
            user_pool_token_account: "pool-account".to_owned(),
        }
    }

    #[test]
    fn create_registers_an_unscored_pump_market() {
        let event = DecodedPumpEvent {
            program: PumpProgram::Pump,
            coordinate: coordinate(0),
            event: PumpEvent::Create(PumpCreateEvent {
                name: "Name".to_owned(),
                symbol: "SYM".to_owned(),
                uri: "https://example.invalid/metadata.json".to_owned(),
                mint: "mint".to_owned(),
                bonding_curve: "curve".to_owned(),
                user: "user".to_owned(),
                creator: "creator".to_owned(),
                timestamp: 100,
                virtual_token_reserves: 1,
                virtual_sol_reserves: 2,
                real_token_reserves: 3,
                token_total_supply: 4,
                token_program: "token-program".to_owned(),
                is_mayhem_mode: false,
                is_cashback_enabled: false,
                quote_mint: "quote".to_owned(),
                virtual_quote_reserves: 5,
            }),
        };

        let normalized = normalize_pump_event(
            event,
            Network::SolanaMainnet,
            Commitment::Confirmed,
            101_000,
            b"evidence",
            &MarketRegistry::default(),
        )
        .expect("create event should normalize");

        assert_eq!(normalized.observation.market.venue, Venue::PumpBondingCurve);
        assert_eq!(
            normalized.observation.source_event_time_unix_ms,
            Some(100_000)
        );
        assert!(normalized.discovered_market.is_some());
        assert_eq!(normalized.observation.raw_evidence_hash.len(), 64);
        assert_eq!(
            normalized.observation.source_evidence_base64,
            "ZXZpZGVuY2U="
        );
    }

    #[test]
    fn trade_uses_registered_market_and_preserves_direction() {
        let registry = MarketRegistry::default();
        registry
            .register(soldisco_domain::MarketIdentity {
                network: Network::SolanaMainnet,
                mint: "mint".to_owned(),
                venue: Venue::PumpBondingCurve,
                market_address: "curve".to_owned(),
                quote_mint: Some("quote".to_owned()),
            })
            .expect("registry should accept Pump market");
        let event = DecodedPumpEvent {
            program: PumpProgram::Pump,
            coordinate: coordinate(1),
            event: PumpEvent::Trade(pump_trade_event(10)),
        };

        let normalized = normalize_pump_event(
            event,
            Network::SolanaMainnet,
            Commitment::Confirmed,
            101_000,
            b"evidence",
            &registry,
        )
        .expect("known trade should normalize");

        assert!(matches!(
            normalized.observation.payload,
            soldisco_domain::ObservationPayload::Trade {
                side: TradeSide::Sell,
                ..
            }
        ));
    }

    #[test]
    fn reversed_pump_swap_create_uses_token_quote_order_but_preserves_source_details() {
        let normalized = normalize_pump_event(
            DecodedPumpEvent {
                program: PumpProgram::PumpSwap,
                coordinate: coordinate(3),
                event: PumpEvent::PumpSwapCreatePool(pump_swap_create_event(
                    WRAPPED_SOL_MINT,
                    "token",
                )),
            },
            Network::SolanaMainnet,
            Commitment::Confirmed,
            103_000,
            b"evidence",
            &MarketRegistry::default(),
        )
        .expect("a reversed wSOL pool should normalize");

        assert_eq!(normalized.observation.market.mint, "token");
        assert_eq!(
            normalized.observation.market.quote_mint.as_deref(),
            Some(WRAPPED_SOL_MINT)
        );
        assert_eq!(
            normalized.observation.payload,
            ObservationPayload::MarketCreated {
                creator: "creator".to_owned(),
                base_amount_units: 22,
                quote_amount_units: 11,
            }
        );
        assert_eq!(
            normalized.observation.source_details["event_type"],
            "PUMP_SWAP_CREATE_POOL"
        );
        assert_eq!(
            normalized.observation.source_details["event"]["base_mint"],
            WRAPPED_SOL_MINT
        );
        assert_eq!(
            normalized.observation.source_details["event"]["quote_mint"],
            "token"
        );
    }

    #[test]
    fn reversed_pump_swap_trades_invert_side_amounts_and_reserves() {
        let registry = MarketRegistry::default();
        let market = registry
            .register_pump_swap_pool(Network::SolanaMainnet, "pool", WRAPPED_SOL_MINT, "token")
            .expect("registry should remain available")
            .expect("reversed wSOL pool should register");
        assert_eq!(market.mint, "token");

        let buy = normalize_pump_event(
            DecodedPumpEvent {
                program: PumpProgram::PumpSwap,
                coordinate: coordinate(4),
                event: PumpEvent::PumpSwapBuy(pump_swap_buy_event()),
            },
            Network::SolanaMainnet,
            Commitment::Confirmed,
            104_000,
            b"buy",
            &registry,
        )
        .expect("source buy should normalize");
        assert_eq!(buy.observation.event_kind, "SELL");
        assert_eq!(
            buy.observation.payload,
            ObservationPayload::Trade {
                side: TradeSide::Sell,
                wallet: "wallet".to_owned(),
                base_amount_units: 22,
                quote_amount_units: 11,
                base_reserve_units: Some(202),
                quote_reserve_units: Some(101),
            }
        );

        let sell = normalize_pump_event(
            DecodedPumpEvent {
                program: PumpProgram::PumpSwap,
                coordinate: coordinate(5),
                event: PumpEvent::PumpSwapSell(pump_swap_sell_event()),
            },
            Network::SolanaMainnet,
            Commitment::Confirmed,
            105_000,
            b"sell",
            &registry,
        )
        .expect("source sell should normalize");
        assert_eq!(sell.observation.event_kind, "BUY");
        assert_eq!(
            sell.observation.payload,
            ObservationPayload::Trade {
                side: TradeSide::Buy,
                wallet: "wallet".to_owned(),
                base_amount_units: 44,
                quote_amount_units: 33,
                base_reserve_units: Some(404),
                quote_reserve_units: Some(303),
            }
        );
    }

    #[test]
    fn standard_pump_swap_trade_orientation_remains_unchanged() {
        let registry = MarketRegistry::default();
        registry
            .register_pump_swap_pool(Network::SolanaMainnet, "pool", "token", WRAPPED_SOL_MINT)
            .expect("registry should remain available")
            .expect("standard wSOL pool should register");

        let normalized = normalize_pump_event(
            DecodedPumpEvent {
                program: PumpProgram::PumpSwap,
                coordinate: coordinate(6),
                event: PumpEvent::PumpSwapBuy(pump_swap_buy_event()),
            },
            Network::SolanaMainnet,
            Commitment::Confirmed,
            106_000,
            b"buy",
            &registry,
        )
        .expect("standard source buy should normalize");

        assert_eq!(
            normalized.observation.payload,
            ObservationPayload::Trade {
                side: TradeSide::Buy,
                wallet: "wallet".to_owned(),
                base_amount_units: 11,
                quote_amount_units: 22,
                base_reserve_units: Some(101),
                quote_reserve_units: Some(202),
            }
        );
    }

    #[test]
    fn pump_swap_liquidity_uses_canonical_token_quote_orientation() {
        let standard_registry = MarketRegistry::default();
        standard_registry
            .register_pump_swap_pool(Network::SolanaMainnet, "pool", "token", WRAPPED_SOL_MINT)
            .expect("registry should remain available")
            .expect("standard wSOL pool should register");
        let deposit = normalize_pump_event(
            DecodedPumpEvent {
                program: PumpProgram::PumpSwap,
                coordinate: coordinate(7),
                event: PumpEvent::PumpSwapDeposit(pump_swap_deposit_event()),
            },
            Network::SolanaMainnet,
            Commitment::Confirmed,
            107_000,
            b"deposit",
            &standard_registry,
        )
        .expect("deposit should normalize");
        assert_eq!(deposit.observation.event_kind, "LIQUIDITY_DEPOSIT");
        assert_eq!(
            deposit.observation.payload,
            ObservationPayload::LiquidityDeposited {
                provider: "liquidity-provider".to_owned(),
                base_amount_units: 11,
                quote_amount_units: 22,
                base_reserve_units: 101,
                quote_reserve_units: 202,
                lp_token_amount_units: 3,
                lp_token_supply_units: 1_000,
            }
        );
        assert_eq!(
            deposit.observation.source_details["event"]["max_quote_amount_in"],
            23
        );

        let reversed_registry = MarketRegistry::default();
        reversed_registry
            .register_pump_swap_pool(Network::SolanaMainnet, "pool", WRAPPED_SOL_MINT, "token")
            .expect("registry should remain available")
            .expect("reversed wSOL pool should register");
        let withdraw = normalize_pump_event(
            DecodedPumpEvent {
                program: PumpProgram::PumpSwap,
                coordinate: coordinate(8),
                event: PumpEvent::PumpSwapWithdraw(pump_swap_withdraw_event()),
            },
            Network::SolanaMainnet,
            Commitment::Confirmed,
            108_000,
            b"withdraw",
            &reversed_registry,
        )
        .expect("withdraw should normalize");
        assert_eq!(withdraw.observation.event_kind, "LIQUIDITY_WITHDRAW");
        assert_eq!(
            withdraw.observation.payload,
            ObservationPayload::LiquidityWithdrawn {
                provider: "liquidity-provider".to_owned(),
                base_amount_units: 44,
                quote_amount_units: 33,
                base_reserve_units: 404,
                quote_reserve_units: 303,
                lp_token_amount_units: 5,
                lp_token_supply_units: 900,
            }
        );
        assert_eq!(withdraw.observation.market.mint, "token");
        assert_eq!(
            withdraw.observation.market.quote_mint.as_deref(),
            Some(WRAPPED_SOL_MINT)
        );
        assert_eq!(
            withdraw.observation.source_details["event"]["min_base_amount_out"],
            32
        );
    }

    #[test]
    fn unsupported_pump_swap_pair_is_rejected_before_observation_creation() {
        let error = normalize_pump_event(
            DecodedPumpEvent {
                program: PumpProgram::PumpSwap,
                coordinate: coordinate(7),
                event: PumpEvent::PumpSwapCreatePool(pump_swap_create_event("token-a", "token-b")),
            },
            Network::SolanaMainnet,
            Commitment::Confirmed,
            107_000,
            b"evidence",
            &MarketRegistry::default(),
        )
        .expect_err("token/token pool must not normalize as a discovery");

        assert!(matches!(
            error,
            NormalizationError::UnsupportedPumpSwapPair { .. }
        ));
    }

    #[test]
    fn zero_value_trade_is_rejected_before_it_can_poison_window_metrics() {
        let registry = MarketRegistry::default();
        registry
            .register(soldisco_domain::MarketIdentity {
                network: Network::SolanaMainnet,
                mint: "mint".to_owned(),
                venue: Venue::PumpBondingCurve,
                market_address: "curve".to_owned(),
                quote_mint: Some("quote".to_owned()),
            })
            .expect("registry should accept Pump market");
        let event = DecodedPumpEvent {
            program: PumpProgram::Pump,
            coordinate: coordinate(2),
            event: PumpEvent::Trade(pump_trade_event(0)),
        };

        let error = normalize_pump_event(
            event,
            Network::SolanaMainnet,
            Commitment::Confirmed,
            101_000,
            b"evidence",
            &registry,
        )
        .expect_err("zero quote amount must be quarantined");

        assert!(matches!(error, NormalizationError::ZeroTradeAmount));
    }

    #[test]
    fn completed_pump_market_can_be_retired_from_the_live_registry() {
        let registry = MarketRegistry::default();
        registry
            .register(soldisco_domain::MarketIdentity {
                network: Network::SolanaMainnet,
                mint: "mint".to_owned(),
                venue: Venue::PumpBondingCurve,
                market_address: "curve".to_owned(),
                quote_mint: Some("quote".to_owned()),
            })
            .expect("registry should accept Pump market");

        registry
            .retire_pump_market("mint")
            .expect("registry should remain writable");
        assert!(registry.pump_market("mint").is_err());
    }
}
