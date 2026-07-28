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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum MarketLookupKey {
    PumpMint(String),
    PumpSwapPool(String),
}

#[derive(Clone, Default)]
pub struct MarketRegistry {
    markets: Arc<RwLock<HashMap<MarketLookupKey, MarketIdentity>>>,
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
    #[error("Pump trade quote mint does not match the registered market for mint {mint}")]
    QuoteMintMismatch { mint: String },
}

impl MarketRegistry {
    pub fn register(&self, market: MarketIdentity) -> Result<(), NormalizationError> {
        let key = match market.venue {
            Venue::PumpBondingCurve => MarketLookupKey::PumpMint(market.mint.clone()),
            Venue::PumpSwap => MarketLookupKey::PumpSwapPool(market.market_address.clone()),
            Venue::RaydiumCpmm | Venue::RaydiumClmm | Venue::RaydiumAmmV4 => return Ok(()),
        };
        self.markets
            .write()
            .map_err(|_| NormalizationError::RegistryUnavailable)?
            .insert(key, market);
        Ok(())
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
            .cloned()
            .ok_or_else(|| NormalizationError::PumpMarketUnresolved(mint.to_owned()))
    }

    fn pump_swap_market(&self, pool: &str) -> Result<MarketIdentity, NormalizationError> {
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
            let market = MarketIdentity {
                network,
                mint: event.base_mint,
                venue: Venue::PumpSwap,
                market_address: event.pool,
                quote_mint: Some(event.quote_mint),
            };
            (
                "CREATE_POOL",
                market.clone(),
                ObservationPayload::MarketCreated {
                    creator: event.creator,
                    base_amount_units: event.base_amount_in,
                    quote_amount_units: event.quote_amount_in,
                },
                Some(market),
            )
        }
        PumpEvent::PumpSwapBuy(event) => (
            "BUY",
            markets.pump_swap_market(&event.pool)?,
            ObservationPayload::Trade {
                side: TradeSide::Buy,
                wallet: event.user,
                base_amount_units: event.base_amount_out,
                quote_amount_units: event.quote_amount_in,
                base_reserve_units: Some(event.pool_base_token_reserves),
                quote_reserve_units: Some(event.pool_quote_token_reserves),
            },
            None,
        ),
        PumpEvent::PumpSwapSell(event) => (
            "SELL",
            markets.pump_swap_market(&event.pool)?,
            ObservationPayload::Trade {
                side: TradeSide::Sell,
                wallet: event.user,
                base_amount_units: event.base_amount_in,
                quote_amount_units: event.quote_amount_out,
                base_reserve_units: Some(event.pool_base_token_reserves),
                quote_reserve_units: Some(event.pool_quote_token_reserves),
            },
            None,
        ),
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
        payload,
    };

    Ok(NormalizedPumpEvent {
        observation,
        discovered_market,
    })
}

fn trade_side(direction: TradeDirection) -> TradeSide {
    match direction {
        TradeDirection::Buy => TradeSide::Buy,
        TradeDirection::Sell => TradeSide::Sell,
    }
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
    use soldisco_domain::{ChainCoordinate, Commitment, Network, TradeSide, Venue};
    use soldisco_source_pump::{
        DecodedPumpEvent, PumpCreateEvent, PumpEvent, PumpProgram, PumpTradeEvent, TradeDirection,
    };

    use super::{MarketRegistry, normalize_pump_event};

    fn coordinate(event_index: u16) -> ChainCoordinate {
        ChainCoordinate {
            slot: 42,
            transaction_index: None,
            signature: "signature".to_owned(),
            instruction_index: 1,
            event_index,
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
            event: PumpEvent::Trade(PumpTradeEvent {
                mint: "mint".to_owned(),
                legacy_sol_amount: 10,
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
                quote_amount: 10,
                virtual_quote_reserves: 1,
                real_quote_reserves: 2,
            }),
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
