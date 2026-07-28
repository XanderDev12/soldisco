use std::{
    env,
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use soldisco_api_contracts::{
    DiscoveryActivity, DiscoveryMode, DiscoveryStage, DiscoveryToken, PrefilterDefaults,
    PrefilterDefaultsValidationError,
};
use soldisco_domain::{
    ChainCoordinate, Commitment, MarketIdentity, Network, NormalizedObservation, ObservationKey,
    ObservationPayload, SourceProgram, TradeSide, Venue,
};
use soldisco_persistence::{
    CollectionPosition, Database, DiscoveryObservation, IntakeQuarantineRecord, NewCollectionGap,
    PersistenceError, PumpSwapPool, RecoveryCheckpoint, RetentionPolicy, WorkFailureDisposition,
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

fn observation(
    market: MarketIdentity,
    program: SourceProgram,
    slot: u64,
    signature: &str,
    payload: ObservationPayload,
) -> NormalizedObservation {
    NormalizedObservation {
        key: ObservationKey {
            network: Network::SolanaMainnet,
            program,
            coordinate: ChainCoordinate {
                slot,
                transaction_index: Some(slot),
                signature: signature.to_owned(),
                instruction_index: 0,
                event_index: 0,
            },
        },
        commitment: Commitment::Confirmed,
        schema_version: 1,
        decoder_version: "integration-decoder-v1".to_owned(),
        event_kind: match &payload {
            ObservationPayload::TokenCreated { .. } => "CREATE",
            ObservationPayload::MarketCreated { .. } => "CREATE_POOL",
            ObservationPayload::Trade {
                side: TradeSide::Buy,
                ..
            } => "BUY",
            ObservationPayload::Trade {
                side: TradeSide::Sell,
                ..
            } => "SELL",
            ObservationPayload::MarketCompleted { .. } => "COMPLETE",
            ObservationPayload::MarketMigrated { .. } => "MIGRATE",
        }
        .to_owned(),
        market,
        source_event_time_unix_ms: Some(i64::try_from(slot).expect("test slot")),
        received_time_unix_ms: i64::try_from(slot).expect("test slot"),
        raw_evidence_hash: format!("hash-{signature}"),
        source_evidence_base64: "dGVzdC1ldmlkZW5jZQ==".to_owned(),
        payload,
    }
}

fn market(
    mint: &str,
    market_address: &str,
    quote_mint: Option<&str>,
    venue: Venue,
) -> MarketIdentity {
    MarketIdentity {
        network: Network::SolanaMainnet,
        mint: mint.to_owned(),
        venue,
        market_address: market_address.to_owned(),
        quote_mint: quote_mint.map(str::to_owned),
    }
}

fn token(
    observation: &NormalizedObservation,
    name: Option<&str>,
    symbol: Option<&str>,
) -> DiscoveryToken {
    let observed_at = observation
        .source_event_time_unix_ms
        .unwrap_or(observation.received_time_unix_ms);
    DiscoveryToken {
        mint: observation.market.mint.clone(),
        name: name.map(str::to_owned),
        symbol: symbol.map(str::to_owned),
        primary_venue: observation.market.venue,
        market_address: observation.market.market_address.clone(),
        quote_mint: observation.market.quote_mint.clone(),
        source_program: observation.key.program,
        stage: DiscoveryStage::Observed,
        last_event_kind: observation.event_kind.clone(),
        observed_slot: observation.key.coordinate.slot,
        first_observed_unix_ms: observed_at,
        last_observed_unix_ms: observed_at,
        latest_signature: observation.key.coordinate.signature.clone(),
        activity: DiscoveryActivity {
            trades: 0,
            buys: 0,
            sells: 0,
            unique_traders: 0,
            base_volume_units: "0".to_owned(),
            quote_volume_units: "0".to_owned(),
        },
        risk_score: None,
        opportunity_score: None,
    }
}

async fn persist(
    database: &Database,
    observation: &NormalizedObservation,
    name: Option<&str>,
    symbol: Option<&str>,
) {
    assert!(
        database
            .insert_observation(observation)
            .await
            .expect("observation insert")
    );
    let mut claimed = database
        .claim_observation_work(
            "DISCOVERY",
            "integration-worker",
            1,
            Duration::from_secs(30),
        )
        .await
        .expect("work claim");
    assert_eq!(claimed.len(), 1);
    let work = claimed.remove(0);
    assert_eq!(work.observation, *observation);
    database
        .commit_discovery_observation(
            work.work_id,
            "integration-worker",
            &DiscoveryObservation {
                token: token(observation, name, symbol),
                validation_source: "PUMP_STRUCTURAL_DECODER".to_owned(),
                validation_version: "1".to_owned(),
            },
        )
        .await
        .expect("projection commit");
}

#[tokio::test]
async fn durable_pipeline_is_idempotent_recoverable_and_market_scoped() {
    let database_url = match env::var("SOLDISCO_TEST_DATABASE_URL") {
        Ok(database_url) => database_url,
        Err(_) if env::var_os("CI").is_some() => {
            panic!("CI must provide SOLDISCO_TEST_DATABASE_URL for PostgreSQL integration")
        }
        Err(_) => {
            eprintln!(
                "SOLDISCO_TEST_DATABASE_URL is unset; explicitly skipping optional local PostgreSQL integration"
            );
            return;
        }
    };
    let schema = format!(
        "soldisco_integration_{}_{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_nanos()
    );
    let administration_pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&database_url)
        .await
        .expect("test database administration connection");
    sqlx::query(&format!("CREATE SCHEMA \"{schema}\""))
        .execute(&administration_pool)
        .await
        .expect("isolated test schema");
    let connect_options = PgConnectOptions::from_str(&database_url)
        .expect("valid PostgreSQL test URL")
        .options([("search_path", schema.as_str())]);
    let database = Database::connect_with_options(connect_options, 5)
        .await
        .expect("test database connection");
    database.migrate().await.expect("migrations");
    database
        .bind_network(Network::SolanaMainnet)
        .await
        .expect("mainnet database binding");
    assert_eq!(
        database.bound_network().await.expect("bound network"),
        Some(Network::SolanaMainnet)
    );
    assert!(matches!(
        database.bind_network(Network::SolanaDevnet).await,
        Err(PersistenceError::DatabaseNetworkMismatch {
            bound: Network::SolanaMainnet,
            requested: Network::SolanaDevnet,
        })
    ));

    let initial_prefilter_defaults = PrefilterDefaults {
        max_event_age_ms: 15_000,
        observation_window_ms: 60_000,
        max_active_windows: 128,
        rpc_requests_per_second: 1,
        rpc_max_in_flight: 4,
        rpc_request_timeout_ms: 5_000,
        rpc_rate_limit_cooldown_ms: 5_000,
    };
    let initial_prefilter = database
        .initialize_prefilter_defaults(initial_prefilter_defaults)
        .await
        .expect("initial prefilter defaults");
    assert_eq!(initial_prefilter.revision, 1);
    assert_eq!(initial_prefilter.values, initial_prefilter_defaults);

    let changed_prefilter_defaults = PrefilterDefaults {
        max_event_age_ms: 20_000,
        observation_window_ms: 90_000,
        max_active_windows: 512,
        rpc_requests_per_second: 8,
        rpc_max_in_flight: 16,
        rpc_request_timeout_ms: 4_000,
        rpc_rate_limit_cooldown_ms: 9_000,
    };
    assert_eq!(
        database
            .initialize_prefilter_defaults(changed_prefilter_defaults)
            .await
            .expect("repeat initialization cannot overwrite persisted values"),
        initial_prefilter
    );
    let updated_prefilter = database
        .update_prefilter_defaults(1, changed_prefilter_defaults)
        .await
        .expect("revision-matched prefilter update");
    assert_eq!(updated_prefilter.revision, 2);
    assert_eq!(updated_prefilter.values, changed_prefilter_defaults);
    assert!(matches!(
        database
            .update_prefilter_defaults(1, initial_prefilter_defaults)
            .await,
        Err(PersistenceError::PrefilterDefaultsRevisionConflict {
            expected: 1,
            actual: Some(2),
        })
    ));
    let mut invalid_prefilter_defaults = changed_prefilter_defaults;
    invalid_prefilter_defaults.rpc_request_timeout_ms =
        invalid_prefilter_defaults.max_event_age_ms + 1;
    assert!(matches!(
        database
            .update_prefilter_defaults(2, invalid_prefilter_defaults)
            .await,
        Err(PersistenceError::InvalidPrefilterDefaults(
            PrefilterDefaultsValidationError::RpcTimeoutExceedsMaximumEventAge
        ))
    ));
    assert_eq!(
        database
            .load_prefilter_defaults()
            .await
            .expect("invalid update leaves prior revision intact"),
        updated_prefilter
    );

    let created = observation(
        market(
            "mint-a",
            "bonding-curve-a",
            Some("wrapped-sol"),
            Venue::PumpBondingCurve,
        ),
        SourceProgram::Pump,
        10,
        "signature-create",
        ObservationPayload::TokenCreated {
            name: "Token A".to_owned(),
            symbol: "TOKA".to_owned(),
            uri: "https://example.invalid/token-a.json".to_owned(),
            creator: "creator".to_owned(),
            user: "user".to_owned(),
        },
    );
    persist(&database, &created, Some("Token A"), Some("TOKA")).await;
    assert!(
        !database
            .insert_observation(&created)
            .await
            .expect("deduplicated replay")
    );

    for (slot, signature, side, base, quote) in [
        (11, "signature-buy", TradeSide::Buy, 5, 10),
        (12, "signature-sell", TradeSide::Sell, 2, 4),
    ] {
        let trade = observation(
            market(
                "mint-a",
                "bonding-curve-a",
                Some("wrapped-sol"),
                Venue::PumpBondingCurve,
            ),
            SourceProgram::Pump,
            slot,
            signature,
            ObservationPayload::Trade {
                side,
                wallet: "wallet-a".to_owned(),
                base_amount_units: base,
                quote_amount_units: quote,
                base_reserve_units: None,
                quote_reserve_units: None,
            },
        );
        persist(&database, &trade, Some("Token A"), Some("TOKA")).await;
    }

    let pump_snapshot = database
        .load_discovery_snapshot()
        .await
        .expect("Pump snapshot");
    assert_eq!(pump_snapshot.tokens[0].activity.trades, 2);
    assert_eq!(pump_snapshot.tokens[0].activity.unique_traders, 1);
    assert_eq!(pump_snapshot.tokens[0].activity.base_volume_units, "7");
    assert_eq!(pump_snapshot.tokens[0].activity.quote_volume_units, "14");
    let mut pump_approved = pump_snapshot.tokens[0].clone();
    pump_approved.stage = DiscoveryStage::Approved;
    database
        .commit_discovery_approval(&pump_approved, "STRUCTURAL_PASS", "1")
        .await
        .expect("Pump approval");

    let pool = PumpSwapPool {
        network: Network::SolanaMainnet,
        pool_address: "pump-swap-pool-a".to_owned(),
        base_mint: "mint-a".to_owned(),
        quote_mint: "wrapped-sol".to_owned(),
        observed_slot: 20,
        observed_signature: "signature-pool".to_owned(),
    };
    assert!(
        database
            .upsert_pump_swap_pool(&pool)
            .await
            .expect("pool upsert")
    );
    assert_eq!(
        database
            .load_pump_swap_pool(Network::SolanaMainnet, "pump-swap-pool-a")
            .await
            .expect("pool lookup"),
        Some(pool.clone())
    );
    assert_eq!(
        database
            .load_pump_swap_pools(Network::SolanaMainnet)
            .await
            .expect("pool registry hydration")
            .len(),
        1
    );
    let mut conflicting_pool = pool.clone();
    conflicting_pool.base_mint = "different-mint".to_owned();
    assert!(matches!(
        database.upsert_pump_swap_pool(&conflicting_pool).await,
        Err(soldisco_persistence::PersistenceError::PumpSwapPoolIdentityConflict { .. })
    ));

    let swap_trade = observation(
        market(
            "mint-a",
            "pump-swap-pool-a",
            Some("wrapped-sol"),
            Venue::PumpSwap,
        ),
        SourceProgram::PumpSwap,
        21,
        "signature-swap-buy",
        ObservationPayload::Trade {
            side: TradeSide::Buy,
            wallet: "wallet-a".to_owned(),
            base_amount_units: 100,
            quote_amount_units: 200,
            base_reserve_units: None,
            quote_reserve_units: None,
        },
    );
    persist(&database, &swap_trade, Some("Token A"), Some("TOKA")).await;

    let swap_token = database
        .load_discovery_token("mint-a")
        .await
        .expect("single token lookup")
        .expect("retained token");
    assert_eq!(swap_token.primary_venue, Venue::PumpSwap);
    assert_eq!(swap_token.stage, DiscoveryStage::Observed);
    assert_eq!(swap_token.market_address, "pump-swap-pool-a");
    assert_eq!(swap_token.quote_mint.as_deref(), Some("wrapped-sol"));
    assert_eq!(swap_token.activity.trades, 1);
    assert_eq!(swap_token.activity.base_volume_units, "100");
    assert_eq!(
        database
            .load_discovery_snapshot()
            .await
            .expect("demoted snapshot")
            .counters
            .approved,
        0
    );

    let stale_pump_trade = observation(
        market(
            "mint-a",
            "bonding-curve-a",
            Some("wrapped-sol"),
            Venue::PumpBondingCurve,
        ),
        SourceProgram::Pump,
        9,
        "signature-stale-pump-buy",
        ObservationPayload::Trade {
            side: TradeSide::Buy,
            wallet: "wallet-b".to_owned(),
            base_amount_units: 1_000,
            quote_amount_units: 2_000,
            base_reserve_units: None,
            quote_reserve_units: None,
        },
    );
    persist(&database, &stale_pump_trade, Some("Token A"), Some("TOKA")).await;
    let after_stale = database
        .load_discovery_token("mint-a")
        .await
        .expect("token after stale event")
        .expect("retained token");
    assert_eq!(after_stale.primary_venue, Venue::PumpSwap);
    assert_eq!(after_stale.activity.trades, 1);
    assert_eq!(after_stale.activity.base_volume_units, "100");

    assert!(
        database
            .set_discovery_mode(DiscoveryMode::ApprovedOnly)
            .await
            .expect("mode change")
    );
    assert!(
        database
            .load_discovery_snapshot()
            .await
            .expect("filtered snapshot")
            .tokens
            .is_empty()
    );

    let mut approved = after_stale;
    approved.stage = DiscoveryStage::Approved;
    database
        .commit_discovery_approval(&approved, "STRUCTURAL_PASS", "1")
        .await
        .expect("explicit promotion");
    let approved_snapshot = database
        .load_discovery_snapshot()
        .await
        .expect("approved snapshot");
    assert_eq!(approved_snapshot.tokens.len(), 1);
    assert_eq!(approved_snapshot.counters.approved, 1);

    let checkpoint = RecoveryCheckpoint {
        network: Network::SolanaMainnet,
        source_program: SourceProgram::Pump,
        checkpoint_kind: "COLLECTOR".to_owned(),
        last_slot: 30,
        last_transaction_index: Some(4),
        last_signature: "signature-30".to_owned(),
    };
    assert!(
        database
            .save_recovery_checkpoint(&checkpoint)
            .await
            .expect("new checkpoint")
    );
    let mut stale_checkpoint = checkpoint.clone();
    stale_checkpoint.last_slot = 29;
    stale_checkpoint.last_signature = "signature-29".to_owned();
    assert!(
        !database
            .save_recovery_checkpoint(&stale_checkpoint)
            .await
            .expect("stale checkpoint")
    );
    let mut same_slot_checkpoint = checkpoint.clone();
    same_slot_checkpoint.last_transaction_index = Some(3);
    same_slot_checkpoint.last_signature = "different-signature-in-slot-30".to_owned();
    assert!(
        !database
            .save_recovery_checkpoint(&same_slot_checkpoint)
            .await
            .expect("same-slot checkpoint")
    );
    let mut indexed_same_slot_checkpoint = checkpoint.clone();
    indexed_same_slot_checkpoint.last_transaction_index = Some(5);
    indexed_same_slot_checkpoint.last_signature = "signature-index-5".to_owned();
    assert!(
        database
            .save_recovery_checkpoint(&indexed_same_slot_checkpoint)
            .await
            .expect("indexed same-slot checkpoint")
    );
    let mut unindexed_same_slot_checkpoint = indexed_same_slot_checkpoint.clone();
    unindexed_same_slot_checkpoint.last_transaction_index = None;
    unindexed_same_slot_checkpoint.last_signature = "signature-without-index".to_owned();
    assert!(
        !database
            .save_recovery_checkpoint(&unindexed_same_slot_checkpoint)
            .await
            .expect("unindexed same-slot fallback")
    );
    assert_eq!(
        database
            .load_recovery_checkpoint(Network::SolanaMainnet, SourceProgram::Pump, "COLLECTOR",)
            .await
            .expect("checkpoint load"),
        Some(indexed_same_slot_checkpoint)
    );

    let newer_swap_without_metadata = observation(
        market(
            "mint-c",
            "pump-swap-pool-c",
            Some("wrapped-sol"),
            Venue::PumpSwap,
        ),
        SourceProgram::PumpSwap,
        60,
        "signature-mint-c-newer-trade",
        ObservationPayload::Trade {
            side: TradeSide::Buy,
            wallet: "wallet-c".to_owned(),
            base_amount_units: 9,
            quote_amount_units: 18,
            base_reserve_units: None,
            quote_reserve_units: None,
        },
    );
    persist(&database, &newer_swap_without_metadata, None, None).await;
    let stale_create_with_metadata = observation(
        market(
            "mint-c",
            "bonding-curve-c",
            Some("wrapped-sol"),
            Venue::PumpBondingCurve,
        ),
        SourceProgram::Pump,
        50,
        "signature-mint-c-older-create",
        ObservationPayload::TokenCreated {
            name: "Token C".to_owned(),
            symbol: "TOKC".to_owned(),
            uri: "https://example.invalid/token-c.json".to_owned(),
            creator: "creator-c".to_owned(),
            user: "user-c".to_owned(),
        },
    );
    persist(
        &database,
        &stale_create_with_metadata,
        Some("Token C"),
        Some("TOKC"),
    )
    .await;
    let enriched = database
        .load_discovery_token("mint-c")
        .await
        .expect("enriched token lookup")
        .expect("retained enriched token");
    assert_eq!(enriched.name.as_deref(), Some("Token C"));
    assert_eq!(enriched.symbol.as_deref(), Some("TOKC"));
    assert_eq!(enriched.primary_venue, Venue::PumpSwap);
    assert_eq!(enriched.market_address, "pump-swap-pool-c");
    assert_eq!(enriched.observed_slot, 60);
    assert_eq!(enriched.latest_signature, "signature-mint-c-newer-trade");
    assert_eq!(enriched.activity.trades, 1);
    assert_eq!(enriched.activity.base_volume_units, "9");

    let retained_markets = database
        .load_discovery_markets(Network::SolanaMainnet)
        .await
        .expect("market registry hydration");
    assert!(retained_markets.iter().any(|identity| {
        identity.mint == "mint-c"
            && identity.venue == Venue::PumpSwap
            && identity.market_address == "pump-swap-pool-c"
    }));
    database
        .set_discovery_mode(DiscoveryMode::ObserveAll)
        .await
        .expect("observe-all mode");
    let bounded_snapshot = database
        .load_discovery_snapshot_bounded(1)
        .await
        .expect("bounded browser snapshot");
    assert_eq!(bounded_snapshot.tokens.len(), 1);
    assert!(bounded_snapshot.tokens_total >= 2);
    assert!(bounded_snapshot.tokens_truncated);

    let quarantine = IntakeQuarantineRecord {
        key: ObservationKey {
            network: Network::SolanaMainnet,
            program: SourceProgram::Pump,
            coordinate: ChainCoordinate {
                slot: 70,
                transaction_index: Some(7),
                signature: "signature-quarantine".to_owned(),
                instruction_index: 1,
                event_index: 2,
            },
        },
        decoder_version: "integration-decoder-v1".to_owned(),
        reason_code: "MALFORMED_EVENT".to_owned(),
        reason_detail: "event field length did not match the published IDL".to_owned(),
        evidence_base64: "YmFkLWV2ZW50".to_owned(),
    };
    assert_eq!(
        database
            .record_intake_quarantine(&quarantine)
            .await
            .expect("first quarantine write"),
        1
    );
    assert_eq!(
        database
            .record_intake_quarantine(&quarantine)
            .await
            .expect("deduplicated quarantine write"),
        2
    );
    let stored_quarantine = database
        .load_intake_quarantine(&quarantine.key)
        .await
        .expect("quarantine load")
        .expect("retained quarantine");
    assert_eq!(stored_quarantine.occurrences, 2);
    assert_eq!(
        stored_quarantine.record.key.coordinate.transaction_index,
        Some(7)
    );
    let mut wrong_network_quarantine = quarantine.clone();
    wrong_network_quarantine.key.network = Network::SolanaDevnet;
    wrong_network_quarantine.key.coordinate.signature =
        "signature-wrong-network-quarantine".to_owned();
    assert!(matches!(
        database
            .record_intake_quarantine(&wrong_network_quarantine)
            .await,
        Err(PersistenceError::Database(_))
    ));

    let now_unix_ms = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_millis(),
    )
    .expect("current timestamp fits i64");
    let gap = NewCollectionGap {
        network: Network::SolanaMainnet,
        source_program: SourceProgram::Pump,
        reason_code: "RECOVERY_BOUND_EXHAUSTED".to_owned(),
        prior_checkpoint: CollectionPosition {
            slot: 30,
            transaction_index: Some(5),
            signature: "signature-index-5".to_owned(),
        },
        detected_at_unix_ms: now_unix_ms,
        details: "bounded signature recovery ended before the durable checkpoint".to_owned(),
    };
    let gap_id = database
        .record_collection_gap(&gap)
        .await
        .expect("gap record");
    assert_eq!(
        database
            .record_collection_gap(&gap)
            .await
            .expect("deduplicated gap record"),
        gap_id
    );
    assert_eq!(
        database
            .count_active_collection_gaps(Network::SolanaMainnet, SourceProgram::Pump)
            .await
            .expect("active gap count"),
        1
    );
    let active_gaps = database
        .load_active_collection_gaps(Network::SolanaMainnet, SourceProgram::Pump)
        .await
        .expect("active gaps");
    assert!(active_gaps.iter().any(|active| active.gap_id == gap_id));
    assert!(matches!(
        database
            .resolve_collection_gap(
                gap_id,
                &CollectionPosition {
                    slot: 29,
                    transaction_index: None,
                    signature: "signature-before-gap".to_owned(),
                },
            )
            .await,
        Err(PersistenceError::InvalidGapResolution)
    ));
    assert!(
        database
            .resolve_collection_gap(
                gap_id,
                &CollectionPosition {
                    slot: 80,
                    transaction_index: None,
                    signature: "signature-gap-resolved".to_owned(),
                },
            )
            .await
            .expect("gap resolution")
    );
    assert_eq!(
        database
            .count_active_collection_gaps(Network::SolanaMainnet, SourceProgram::Pump)
            .await
            .expect("resolved gap count"),
        0
    );

    let lease_test = observation(
        market("mint-b", "bonding-curve-b", None, Venue::PumpBondingCurve),
        SourceProgram::Pump,
        40,
        "signature-lease",
        ObservationPayload::TokenCreated {
            name: "Token B".to_owned(),
            symbol: "TOKB".to_owned(),
            uri: "https://example.invalid/token-b.json".to_owned(),
            creator: "creator".to_owned(),
            user: "user".to_owned(),
        },
    );
    assert!(
        database
            .insert_observation(&lease_test)
            .await
            .expect("lease test insert")
    );
    let first_claim = database
        .claim_observation_work("DISCOVERY", "worker-one", 1, Duration::from_millis(1))
        .await
        .expect("first claim")
        .remove(0);
    tokio::time::sleep(Duration::from_millis(10)).await;
    let second_claim = database
        .claim_observation_work("DISCOVERY", "worker-two", 1, Duration::from_secs(30))
        .await
        .expect("expired lease reclaim")
        .remove(0);
    assert_eq!(second_claim.work_id, first_claim.work_id);
    assert_eq!(second_claim.attempts, 2);
    assert_eq!(
        database
            .retry_or_fail_observation_work(
                second_claim.work_id,
                "worker-two",
                "STRUCTURAL_VALIDATION_FAILED",
                Duration::ZERO,
                2,
            )
            .await
            .expect("terminal failure"),
        WorkFailureDisposition::PermanentlyFailed
    );
    let final_snapshot = database
        .load_discovery_snapshot()
        .await
        .expect("final snapshot");
    assert_eq!(final_snapshot.counters.pending, 0);
    assert_eq!(final_snapshot.counters.rejected, 1);

    let retained_pending = observation(
        market(
            "mint-pending",
            "curve-pending",
            None,
            Venue::PumpBondingCurve,
        ),
        SourceProgram::Pump,
        90,
        "signature-retained-pending",
        ObservationPayload::TokenCreated {
            name: "Pending".to_owned(),
            symbol: "PND".to_owned(),
            uri: "https://example.invalid/pending.json".to_owned(),
            creator: "creator".to_owned(),
            user: "user".to_owned(),
        },
    );
    assert!(
        database
            .insert_observation(&retained_pending)
            .await
            .expect("pending retention observation")
    );
    let retained_processing = observation(
        market(
            "mint-processing",
            "curve-processing",
            None,
            Venue::PumpBondingCurve,
        ),
        SourceProgram::Pump,
        91,
        "signature-retained-processing",
        ObservationPayload::TokenCreated {
            name: "Processing".to_owned(),
            symbol: "PRC".to_owned(),
            uri: "https://example.invalid/processing.json".to_owned(),
            creator: "creator".to_owned(),
            user: "user".to_owned(),
        },
    );
    assert!(
        database
            .insert_observation(&retained_processing)
            .await
            .expect("processing retention observation")
    );
    let processing_work = database
        .claim_observation_work(
            "DISCOVERY",
            "processing-retention-verifier",
            1,
            Duration::from_secs(30),
        )
        .await
        .expect("processing work claim")
        .remove(0);
    assert!(database.database_size_bytes().await.expect("database size") > 0);
    let future_cutoff = now_unix_ms + 60_000;
    let pruned = database
        .prune_retained_history(RetentionPolicy {
            terminal_history_before_unix_ms: future_cutoff,
            projection_events_before_unix_ms: future_cutoff,
            quarantine_before_unix_ms: future_cutoff,
            batch_size: 10_000,
        })
        .await
        .expect("bounded retention prune");
    assert!(pruned.terminal_observations > 0);
    assert!(pruned.projection_events > 0);
    assert!(pruned.quarantine_records > 0);
    assert!(
        database
            .load_intake_quarantine(&quarantine.key)
            .await
            .expect("pruned quarantine lookup")
            .is_none()
    );
    database
        .renew_observation_work_lease(
            processing_work.work_id,
            "processing-retention-verifier",
            Duration::from_secs(30),
        )
        .await
        .expect("processing work survives pruning");
    let claimed_pending = database
        .claim_observation_work(
            "DISCOVERY",
            "retention-verifier",
            1_000,
            Duration::from_secs(30),
        )
        .await
        .expect("pending work survives pruning");
    assert!(claimed_pending.iter().any(|work| {
        matches!(
            work.observation.key.coordinate.signature.as_str(),
            "signature-retained-pending" | "signature-retained-processing"
        )
    }));
    let retained_aggregate = database
        .load_discovery_token("mint-c")
        .await
        .expect("aggregate after raw pruning")
        .expect("discovery aggregate is never pruned");
    assert_eq!(retained_aggregate.market_address, "pump-swap-pool-c");

    database.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&administration_pool)
        .await
        .expect("isolated test schema cleanup");
    administration_pool.close().await;
}
