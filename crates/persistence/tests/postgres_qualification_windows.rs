use std::{
    env,
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use soldisco_api_contracts::{
    DiscoveryActivity, DiscoveryMode, DiscoveryStage, DiscoveryToken, PrefilterDefaults,
    QualificationDecision, QualificationDefaults, WindowCompleteness,
};
use soldisco_domain::{
    ChainCoordinate, Commitment, MarketIdentity, Network, NormalizedObservation, ObservationKey,
    ObservationPayload, SourceProgram, TradeSide, Venue,
};
use soldisco_persistence::{
    Database, DiscoveryObservation, DiscoveryProjectionMutation,
    DiscoveryWindowProjectionReadiness, DiscoveryWindowRecord, FinalizeDiscoveryWindow,
    NewDiscoveryWindow, PersistenceError, RetentionPolicy, StoredPrefilterDefaults,
    StoredQualificationDefaults, WindowTargetKind, WorkFailureDisposition,
    build_discovery_window_finalization,
};
use sqlx::{
    PgPool,
    postgres::{PgConnectOptions, PgPoolOptions},
};

const QUOTE_MINT: &str = "So11111111111111111111111111111111111111112";

fn market(
    mint: &str,
    market_address: &str,
    venue: Venue,
    quote_mint: Option<&str>,
) -> MarketIdentity {
    MarketIdentity {
        network: Network::SolanaMainnet,
        mint: mint.to_owned(),
        venue,
        market_address: market_address.to_owned(),
        quote_mint: quote_mint.map(str::to_owned),
    }
}

fn observation(
    market: MarketIdentity,
    program: SourceProgram,
    slot: u64,
    received_time_unix_ms: i64,
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
        decoder_version: "qualification-integration-decoder-v1".to_owned(),
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
            ObservationPayload::LiquidityDeposited { .. } => "DEPOSIT",
            ObservationPayload::LiquidityWithdrawn { .. } => "WITHDRAW",
            ObservationPayload::MarketCompleted { .. } => "COMPLETE",
            ObservationPayload::MarketMigrated { .. } => "MIGRATE",
        }
        .to_owned(),
        market,
        source_event_time_unix_ms: Some(i64::try_from(slot).expect("test slot fits i64")),
        received_time_unix_ms,
        raw_evidence_hash: format!("hash-{signature}"),
        source_evidence_base64: "ZHVyYWJsZS1xdWFsaWZpY2F0aW9uLWZpeHR1cmU=".to_owned(),
        source_details: serde_json::json!({
            "fixture": "durable-qualification",
            "signature": signature,
        }),
        payload,
    }
}

fn discovery_token(observation: &NormalizedObservation) -> DiscoveryToken {
    let (name, symbol) = match &observation.payload {
        ObservationPayload::TokenCreated { name, symbol, .. } => {
            (Some(name.clone()), Some(symbol.clone()))
        }
        ObservationPayload::MarketCreated { .. }
        | ObservationPayload::Trade { .. }
        | ObservationPayload::LiquidityDeposited { .. }
        | ObservationPayload::LiquidityWithdrawn { .. }
        | ObservationPayload::MarketCompleted { .. }
        | ObservationPayload::MarketMigrated { .. } => (None, None),
    };
    let observed_at = observation
        .source_event_time_unix_ms
        .unwrap_or(observation.received_time_unix_ms);
    DiscoveryToken {
        mint: observation.market.mint.clone(),
        name,
        symbol,
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
        qualification: None,
        risk_score: None,
        opportunity_score: None,
    }
}

async fn project_all_pending(database: &Database, worker_id: &str) -> usize {
    let mut projected = 0;
    loop {
        let claimed = database
            .claim_observation_work("DISCOVERY", worker_id, 64, Duration::from_secs(30))
            .await
            .expect("claim qualification fixture work");
        if claimed.is_empty() {
            return projected;
        }
        for work in claimed {
            database
                .commit_discovery_observation(
                    work.work_id,
                    worker_id,
                    &DiscoveryObservation {
                        token: discovery_token(&work.observation),
                        validation_source: "QUALIFICATION_INTEGRATION_FIXTURE".to_owned(),
                        validation_version: "1".to_owned(),
                    },
                )
                .await
                .expect("project qualification fixture");
            projected += 1;
        }
    }
}

fn new_window(
    opening: &NormalizedObservation,
    closes_at_unix_ms: i64,
    collector_run_id: &str,
    prefilter: StoredPrefilterDefaults,
    qualification: StoredQualificationDefaults,
) -> NewDiscoveryWindow {
    NewDiscoveryWindow {
        target_kind: WindowTargetKind::PumpMint,
        target_address: opening.market.mint.clone(),
        opened_at_unix_ms: opening.received_time_unix_ms,
        closes_at_unix_ms,
        collector_run_id: collector_run_id.to_owned(),
        prefilter_revision: prefilter.revision,
        prefilter_values: prefilter.values,
        qualification,
    }
}

fn pass_finalization(
    window: &DiscoveryWindowRecord,
    finalized_at_unix_ms: i64,
    _fixture: &str,
) -> FinalizeDiscoveryWindow {
    let finalized = build_discovery_window_finalization(window, finalized_at_unix_ms, None)
        .expect("canonical qualification finalization");
    assert_eq!(
        finalized.decision,
        soldisco_domain::AssessmentDecision::Pass
    );
    finalized
}

struct ProjectionBarrierFixture<'a> {
    name: &'a str,
    opened_at_unix_ms: i64,
    prefilter: StoredPrefilterDefaults,
    qualification: StoredQualificationDefaults,
    incomplete_reason: Option<&'a str>,
    expected_decision: QualificationDecision,
}

async fn assert_terminal_finalization_waits_for_projection(
    database: &Database,
    verification_pool: &PgPool,
    fixture: ProjectionBarrierFixture<'_>,
) {
    let fixture_market = market(
        &format!("qualification-mint-{}", fixture.name),
        &format!("qualification-bonding-curve-{}", fixture.name),
        Venue::PumpBondingCurve,
        Some(QUOTE_MINT),
    );
    let opening_slot = u64::try_from(fixture.opened_at_unix_ms).expect("test time fits u64");
    let opening = observation(
        fixture_market.clone(),
        SourceProgram::Pump,
        opening_slot,
        fixture.opened_at_unix_ms,
        &format!("qualification-{}-create", fixture.name),
        ObservationPayload::TokenCreated {
            name: format!("Qualification {}", fixture.name),
            symbol: format!("Q{}", fixture.name),
            uri: format!(
                "https://example.invalid/qualification-{}.json",
                fixture.name
            ),
            creator: format!("creator-{}", fixture.name),
            user: format!("user-{}", fixture.name),
        },
    );
    let closes_at_unix_ms = fixture.opened_at_unix_ms + 60_000;
    let opened = database
        .open_discovery_window(
            &opening,
            &new_window(
                &opening,
                closes_at_unix_ms,
                &format!("qualification-run-{}", fixture.name),
                fixture.prefilter,
                fixture.qualification,
            ),
        )
        .await
        .expect("open projection-barrier window");
    assert_eq!(
        database
            .load_discovery_window_projection_readiness(opened.window_id)
            .await
            .expect("opening projection readiness"),
        DiscoveryWindowProjectionReadiness::Pending
    );
    assert_eq!(
        project_all_pending(
            database,
            &format!("qualification-{}-opening-worker", fixture.name),
        )
        .await,
        1,
        "the opening observation must be terminal before adding later evidence"
    );
    assert_eq!(
        database
            .load_discovery_window_projection_readiness(opened.window_id)
            .await
            .expect("settled opening projection readiness"),
        DiscoveryWindowProjectionReadiness::Ready
    );

    let trade = observation(
        fixture_market,
        SourceProgram::Pump,
        opening_slot + 1,
        fixture.opened_at_unix_ms + 1_000,
        &format!("qualification-{}-later-buy", fixture.name),
        ObservationPayload::Trade {
            side: TradeSide::Buy,
            wallet: format!("wallet-{}", fixture.name),
            base_amount_units: 10,
            quote_amount_units: 20,
            base_reserve_units: None,
            quote_reserve_units: None,
        },
    );
    assert!(
        database
            .insert_window_observation(opened.window_id, &trade)
            .await
            .expect("insert later projection-barrier evidence")
    );
    assert_eq!(
        database
            .load_discovery_window_projection_readiness(opened.window_id)
            .await
            .expect("later projection readiness"),
        DiscoveryWindowProjectionReadiness::Pending
    );

    let evidence = database
        .load_discovery_window(opened.window_id)
        .await
        .expect("load projection-barrier evidence");
    let finalization = build_discovery_window_finalization(
        &evidence,
        closes_at_unix_ms + 1,
        fixture.incomplete_reason,
    )
    .expect("canonical terminal finalization");
    assert_eq!(finalization.summary.decision, fixture.expected_decision);

    assert!(matches!(
        database.finalize_discovery_window(&finalization).await,
        Err(PersistenceError::DiscoveryWindowProjectionMissing { window_id })
            if window_id == opened.window_id
    ));
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM discovery_windows WHERE id = $1")
            .bind(opened.window_id)
            .fetch_one(verification_pool)
            .await
            .expect("active status after pending projection barrier"),
        "ACTIVE"
    );

    let worker_id = format!("qualification-{}-later-worker", fixture.name);
    let claimed = database
        .claim_observation_work("DISCOVERY", &worker_id, 8, Duration::from_secs(30))
        .await
        .expect("claim later projection-barrier work");
    assert_eq!(claimed.len(), 1);
    assert_eq!(
        claimed[0].observation.key.coordinate.signature,
        trade.key.coordinate.signature
    );
    assert_eq!(
        database
            .load_discovery_window_projection_readiness(opened.window_id)
            .await
            .expect("leased projection readiness"),
        DiscoveryWindowProjectionReadiness::Pending
    );
    assert!(matches!(
        database.finalize_discovery_window(&finalization).await,
        Err(PersistenceError::DiscoveryWindowProjectionMissing { window_id })
            if window_id == opened.window_id
    ));

    let claimed = claimed.into_iter().next().expect("one claimed work item");
    database
        .commit_discovery_observation(
            claimed.work_id,
            &worker_id,
            &DiscoveryObservation {
                token: discovery_token(&claimed.observation),
                validation_source: "QUALIFICATION_PROJECTION_BARRIER_FIXTURE".to_owned(),
                validation_version: "1".to_owned(),
            },
        )
        .await
        .expect("finish later projection-barrier work");
    assert_eq!(
        database
            .load_discovery_window_projection_readiness(opened.window_id)
            .await
            .expect("terminal projection readiness"),
        DiscoveryWindowProjectionReadiness::Ready
    );
    assert!(
        database
            .finalize_discovery_window(&finalization)
            .await
            .expect("retry terminal finalization after projection completes")
    );

    let terminal = database
        .load_discovery_token(&opening.market.mint)
        .await
        .expect("load terminal projection-barrier candidate")
        .expect("terminal candidate retained");
    assert_eq!(terminal.stage, DiscoveryStage::Observed);
    assert_eq!(
        terminal.qualification.as_ref(),
        Some(&finalization.summary),
        "the retry must retain the exact canonical terminal summary"
    );
}

async fn assert_terminal_opening_projection_is_not_treated_as_ready(
    database: &Database,
    verification_pool: &PgPool,
    prefilter: StoredPrefilterDefaults,
    qualification: StoredQualificationDefaults,
) {
    let failed_market = market(
        "qualification-mint-opening-failed",
        "qualification-bonding-curve-opening-failed",
        Venue::PumpBondingCurve,
        Some(QUOTE_MINT),
    );
    let opening = observation(
        failed_market.clone(),
        SourceProgram::Pump,
        200_000,
        200_000,
        "qualification-opening-failed-create",
        ObservationPayload::TokenCreated {
            name: "Qualification Opening Failed".to_owned(),
            symbol: "QFAIL".to_owned(),
            uri: "https://example.invalid/qualification-opening-failed.json".to_owned(),
            creator: "creator-opening-failed".to_owned(),
            user: "user-opening-failed".to_owned(),
        },
    );
    let window = database
        .open_discovery_window(
            &opening,
            &new_window(
                &opening,
                260_000,
                "qualification-run-opening-failed",
                prefilter,
                qualification,
            ),
        )
        .await
        .expect("open failed-opening readiness window");
    let worker_id = "qualification-opening-failed-worker";
    let claimed = database
        .claim_observation_work("DISCOVERY", worker_id, 1, Duration::from_secs(30))
        .await
        .expect("claim opening projection that will fail");
    assert_eq!(claimed.len(), 1);
    assert_eq!(
        database
            .retry_or_fail_observation_work(
                claimed[0].work_id,
                worker_id,
                "OPENING_PROJECTION_FIXTURE_FAILURE",
                Duration::ZERO,
                1,
            )
            .await
            .expect("terminally fail opening projection"),
        WorkFailureDisposition::PermanentlyFailed
    );
    assert_eq!(
        database
            .load_discovery_window_projection_readiness(window.window_id)
            .await
            .expect("failed opening readiness"),
        DiscoveryWindowProjectionReadiness::OpeningProjectionUnavailable
    );

    for (slot, signature, side, wallet) in [
        (
            210_000,
            "qualification-opening-failed-buy",
            TradeSide::Buy,
            "wallet-opening-failed-a",
        ),
        (
            220_000,
            "qualification-opening-failed-sell",
            TradeSide::Sell,
            "wallet-opening-failed-b",
        ),
    ] {
        let trade = observation(
            failed_market.clone(),
            SourceProgram::Pump,
            slot,
            i64::try_from(slot).expect("fixture slot fits i64"),
            signature,
            ObservationPayload::Trade {
                side,
                wallet: wallet.to_owned(),
                base_amount_units: 10,
                quote_amount_units: 20,
                base_reserve_units: None,
                quote_reserve_units: None,
            },
        );
        assert!(
            database
                .insert_window_observation(window.window_id, &trade)
                .await
                .expect("insert activity after failed opening projection")
        );
    }
    assert_eq!(
        project_all_pending(database, "qualification-opening-failed-activity-worker").await,
        2
    );
    assert_eq!(
        database
            .load_discovery_window_projection_readiness(window.window_id)
            .await
            .expect("failed opening remains unavailable after activity projection"),
        DiscoveryWindowProjectionReadiness::OpeningProjectionUnavailable
    );

    let evidence = database
        .load_discovery_window(window.window_id)
        .await
        .expect("failed-opening window evidence");
    let complete =
        build_discovery_window_finalization(&evidence, 260_001, None).expect("complete evidence");
    assert_eq!(
        complete.summary.decision,
        QualificationDecision::Pass,
        "activity alone would pass if the missing opening projection were ignored"
    );
    assert!(matches!(
        database.finalize_discovery_window(&complete).await,
        Err(PersistenceError::DiscoveryWindowFinalizationEvidenceMismatch { window_id })
            if window_id == window.window_id
    ));
    let incomplete =
        build_discovery_window_finalization(&evidence, 260_001, Some("PROCESSING_FAILED"))
            .expect("processing-failed evidence");
    assert_eq!(incomplete.summary.decision, QualificationDecision::Unknown);
    assert!(
        database
            .finalize_discovery_window(&incomplete)
            .await
            .expect("finalize failed-opening window as incomplete")
    );
    let terminal = database
        .load_discovery_token(&opening.market.mint)
        .await
        .expect("load failed-opening candidate")
        .expect("activity projection created a candidate");
    assert_eq!(
        terminal
            .qualification
            .as_ref()
            .expect("terminal qualification")
            .completeness,
        WindowCompleteness::Incomplete
    );

    let missing_opening = observation(
        market(
            "qualification-mint-opening-missing",
            "qualification-bonding-curve-opening-missing",
            Venue::PumpBondingCurve,
            Some(QUOTE_MINT),
        ),
        SourceProgram::Pump,
        300_000,
        300_000,
        "qualification-opening-missing-create",
        ObservationPayload::TokenCreated {
            name: "Qualification Opening Missing".to_owned(),
            symbol: "QMISS".to_owned(),
            uri: "https://example.invalid/qualification-opening-missing.json".to_owned(),
            creator: "creator-opening-missing".to_owned(),
            user: "user-opening-missing".to_owned(),
        },
    );
    let missing_window = database
        .open_discovery_window(
            &missing_opening,
            &new_window(
                &missing_opening,
                360_000,
                "qualification-run-opening-missing",
                prefilter,
                qualification,
            ),
        )
        .await
        .expect("open missing-opening readiness window");
    sqlx::query(
        "UPDATE observation_work \
         SET status = 'COMPLETE', finished_at = NOW(), updated_at = NOW() \
         WHERE observation_id = (\
             SELECT opening_observation_id \
             FROM discovery_windows \
             WHERE id = $1\
         ) AND work_kind = 'DISCOVERY'",
    )
    .bind(missing_window.window_id)
    .execute(verification_pool)
    .await
    .expect("simulate terminal opening work with missing projection");
    assert_eq!(
        database
            .load_discovery_window_projection_readiness(missing_window.window_id)
            .await
            .expect("missing opening projection readiness"),
        DiscoveryWindowProjectionReadiness::OpeningProjectionUnavailable
    );
}

async fn wait_for_postgres_lock_wait(
    verification_pool: &PgPool,
    application_name: &str,
    wait_event: Option<&str>,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let is_waiting = sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS (\
                    SELECT 1 \
                    FROM pg_stat_activity \
                    WHERE application_name = $1 \
                      AND state = 'active' \
                      AND wait_event_type = 'Lock' \
                      AND ($2::TEXT IS NULL OR wait_event = $2)\
                 )",
            )
            .bind(application_name)
            .bind(wait_event)
            .fetch_one(verification_pool)
            .await
            .expect("inspect PostgreSQL lock waiter");
            if is_waiting {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("database operation reached the expected lock wait");
}

#[tokio::test]
async fn reject_and_unknown_wait_for_all_same_window_discovery_work() {
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
        "soldisco_projection_barrier_{}_{}",
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
        .expect("isolated projection-barrier schema");
    let connect_options = PgConnectOptions::from_str(&database_url)
        .expect("valid PostgreSQL test URL")
        .options([("search_path", schema.as_str())]);
    let verification_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(connect_options.clone())
        .await
        .expect("projection-barrier verification connection");
    let database = Database::connect_with_options(connect_options, 5)
        .await
        .expect("projection-barrier database connection");
    database.migrate().await.expect("migrations");
    database
        .set_discovery_mode(DiscoveryMode::ObserveAll)
        .await
        .expect("observe-all integration projection");
    database
        .bind_network(Network::SolanaMainnet)
        .await
        .expect("mainnet database binding");

    let prefilter = database
        .initialize_prefilter_defaults(PrefilterDefaults {
            max_event_age_ms: 15_000,
            observation_window_ms: 60_000,
            max_active_windows: 128,
            rpc_requests_per_second: 8,
            rpc_max_in_flight: 16,
            rpc_request_timeout_ms: 5_000,
            rpc_rate_limit_cooldown_ms: 5_000,
        })
        .await
        .expect("initial durable prefilter defaults");
    let qualification = database
        .initialize_qualification_defaults(QualificationDefaults {
            minimum_trades: 2,
            minimum_unique_traders: 2,
            minimum_buys: 1,
            minimum_sells: 1,
            minimum_native_quote_volume_units: 1,
            minimum_stable_quote_volume_units: 1,
            maximum_single_wallet_quote_share_bps: 10_000,
        })
        .await
        .expect("initial durable qualification defaults");

    assert_terminal_finalization_waits_for_projection(
        &database,
        &verification_pool,
        ProjectionBarrierFixture {
            name: "reject",
            opened_at_unix_ms: 10_000,
            prefilter,
            qualification,
            incomplete_reason: None,
            expected_decision: QualificationDecision::Reject,
        },
    )
    .await;
    assert_terminal_finalization_waits_for_projection(
        &database,
        &verification_pool,
        ProjectionBarrierFixture {
            name: "unknown",
            opened_at_unix_ms: 100_000,
            prefilter,
            qualification,
            incomplete_reason: Some("PIPELINE_RESTARTED"),
            expected_decision: QualificationDecision::Unknown,
        },
    )
    .await;
    assert_terminal_opening_projection_is_not_treated_as_ready(
        &database,
        &verification_pool,
        prefilter,
        qualification,
    )
    .await;

    database.close().await;
    verification_pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&administration_pool)
        .await
        .expect("isolated projection-barrier schema cleanup");
    administration_pool.close().await;
}

#[tokio::test]
async fn durable_qualification_windows_are_retryable_idempotent_and_projection_safe() {
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
        "soldisco_qualification_integration_{}_{}",
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
        .expect("isolated qualification schema");
    let connect_options = PgConnectOptions::from_str(&database_url)
        .expect("valid PostgreSQL test URL")
        .options([("search_path", schema.as_str())]);
    let verification_pool = PgPoolOptions::new()
        .max_connections(2)
        .connect_with(connect_options.clone())
        .await
        .expect("qualification verification connection");
    let database = Database::connect_with_options(connect_options, 5)
        .await
        .expect("qualification database connection");
    database.migrate().await.expect("migrations");
    database
        .set_discovery_mode(DiscoveryMode::ObserveAll)
        .await
        .expect("observe-all integration projection");
    database
        .bind_network(Network::SolanaMainnet)
        .await
        .expect("mainnet database binding");

    let prefilter = database
        .initialize_prefilter_defaults(PrefilterDefaults {
            max_event_age_ms: 15_000,
            observation_window_ms: 60_000,
            max_active_windows: 128,
            rpc_requests_per_second: 8,
            rpc_max_in_flight: 16,
            rpc_request_timeout_ms: 5_000,
            rpc_rate_limit_cooldown_ms: 5_000,
        })
        .await
        .expect("initial durable prefilter defaults");
    let qualification_v1 = database
        .initialize_qualification_defaults(QualificationDefaults {
            minimum_trades: 2,
            minimum_unique_traders: 2,
            minimum_buys: 1,
            minimum_sells: 1,
            minimum_native_quote_volume_units: 1,
            minimum_stable_quote_volume_units: 1,
            maximum_single_wallet_quote_share_bps: 10_000,
        })
        .await
        .expect("initial durable qualification defaults");

    let primary_market = market(
        "qualification-mint-primary",
        "qualification-bonding-curve-primary",
        Venue::PumpBondingCurve,
        Some(QUOTE_MINT),
    );
    let opening = observation(
        primary_market.clone(),
        SourceProgram::Pump,
        10_000,
        10_000,
        "qualification-primary-create",
        ObservationPayload::TokenCreated {
            name: "Qualification Primary".to_owned(),
            symbol: "QPRI".to_owned(),
            uri: "https://example.invalid/qualification-primary.json".to_owned(),
            creator: "creator-primary".to_owned(),
            user: "user-primary".to_owned(),
        },
    );
    let primary_window = database
        .open_discovery_window(
            &opening,
            &new_window(
                &opening,
                70_000,
                "qualification-run-primary",
                prefilter,
                qualification_v1,
            ),
        )
        .await
        .expect("open primary qualification window");
    assert!(primary_window.observation_inserted);
    assert!(primary_window.window_created);
    assert!(!primary_window.replayed);
    let counters_after_open = database
        .load_discovery_snapshot()
        .await
        .expect("counters after opening primary window")
        .counters;
    assert_eq!(counters_after_open.qualification_pending, 1);
    assert_eq!(counters_after_open.pending, 1);

    let mut qualification_v2_values = qualification_v1.values;
    qualification_v2_values.maximum_single_wallet_quote_share_bps = 9_000;
    let qualification_v2 = database
        .update_qualification_defaults(qualification_v1.revision, qualification_v2_values)
        .await
        .expect("append qualification revision two");
    assert_eq!(qualification_v2.revision, qualification_v1.revision + 1);

    let replayed_opening = opening.clone();
    let replay = database
        .open_discovery_window(
            &replayed_opening,
            &new_window(
                &replayed_opening,
                70_001,
                "qualification-run-replayed",
                prefilter,
                qualification_v2,
            ),
        )
        .await
        .expect("exact opening replay resolves to the durable window");
    assert_eq!(replay.window_id, primary_window.window_id);
    assert!(!replay.observation_inserted);
    assert!(!replay.window_created);
    assert!(replay.replayed);
    let counters_after_replay = database
        .load_discovery_snapshot()
        .await
        .expect("replay counters")
        .counters;
    assert_eq!(
        counters_after_replay.qualification_pending,
        counters_after_open.qualification_pending
    );
    assert_eq!(counters_after_replay.pending, counters_after_open.pending);

    let membership_count_before = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM discovery_window_observations WHERE window_id = $1",
    )
    .bind(primary_window.window_id)
    .fetch_one(&verification_pool)
    .await
    .expect("membership count before transport replay");
    let work_count_before = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) \
         FROM observation_work AS work \
         JOIN chain_observations AS observation ON observation.id = work.observation_id \
         WHERE observation.signature = 'qualification-primary-create'",
    )
    .fetch_one(&verification_pool)
    .await
    .expect("work count before transport replay");

    let mut later_transport_replay = opening.clone();
    later_transport_replay.received_time_unix_ms += 1;
    later_transport_replay.commitment = Commitment::Finalized;
    later_transport_replay.key.coordinate.transaction_index = None;
    let later_replay = database
        .open_discovery_window(
            &later_transport_replay,
            &new_window(
                &later_transport_replay,
                70_001,
                "qualification-run-transport-replay",
                prefilter,
                qualification_v2,
            ),
        )
        .await
        .expect("later transport replay resolves to the original durable window");
    assert_eq!(later_replay.window_id, primary_window.window_id);
    assert!(!later_replay.observation_inserted);
    assert!(!later_replay.window_created);
    assert!(later_replay.replayed);

    let membership_count_after = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM discovery_window_observations WHERE window_id = $1",
    )
    .bind(primary_window.window_id)
    .fetch_one(&verification_pool)
    .await
    .expect("membership count after transport replay");
    let work_count_after = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) \
         FROM observation_work AS work \
         JOIN chain_observations AS observation ON observation.id = work.observation_id \
         WHERE observation.signature = 'qualification-primary-create'",
    )
    .fetch_one(&verification_pool)
    .await
    .expect("work count after transport replay");
    assert_eq!(membership_count_after, membership_count_before);
    assert_eq!(work_count_after, work_count_before);

    let evidence_after_transport_replay = database
        .load_discovery_window(primary_window.window_id)
        .await
        .expect("load evidence after transport replay");
    assert_eq!(
        evidence_after_transport_replay.observations,
        vec![opening.clone()],
        "transport replay must preserve the original receipt and evidence"
    );
    let mut earlier_opening_replay = opening.clone();
    earlier_opening_replay.received_time_unix_ms -= 1;
    earlier_opening_replay.commitment = Commitment::Finalized;
    earlier_opening_replay.key.coordinate.transaction_index = None;
    assert!(
        !database
            .insert_observation(&earlier_opening_replay)
            .await
            .expect("earlier replay of a window opener remains idempotent")
    );
    let opening_times = sqlx::query_as::<_, (i64, i64, i64, i64)>(
        "SELECT observation.received_time_unix_ms, \
                member.admitted_at_unix_ms, \
                discovery_window.opened_at_unix_ms, \
                discovery_window.closes_at_unix_ms \
         FROM discovery_windows AS discovery_window \
         JOIN chain_observations AS observation \
           ON observation.id = discovery_window.opening_observation_id \
         JOIN discovery_window_observations AS member \
           ON member.window_id = discovery_window.id \
          AND member.observation_id = observation.id \
         WHERE discovery_window.id = $1",
    )
    .bind(primary_window.window_id)
    .fetch_one(&verification_pool)
    .await
    .expect("opening times after earlier transport replay");
    assert_eq!(
        opening_times,
        (10_000, 10_000, 10_000, 70_000),
        "an existing window freezes its opening receipt and bounds"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM discovery_window_observations WHERE window_id = $1",
        )
        .bind(primary_window.window_id)
        .fetch_one(&verification_pool)
        .await
        .expect("opening membership count after earlier replay"),
        membership_count_before
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) \
             FROM observation_work AS work \
             JOIN chain_observations AS observation ON observation.id = work.observation_id \
             WHERE observation.signature = 'qualification-primary-create'",
        )
        .fetch_one(&verification_pool)
        .await
        .expect("opening work count after earlier replay"),
        work_count_before
    );

    let pinned_window = database
        .load_discovery_window(primary_window.window_id)
        .await
        .expect("load revision-pinned primary window");
    assert_eq!(
        pinned_window.qualification_revision,
        qualification_v1.revision
    );
    assert_eq!(pinned_window.qualification_values, qualification_v1.values);
    assert_ne!(
        pinned_window.qualification_values, qualification_v2.values,
        "an open window must not adopt later operator settings"
    );

    let close_boundary_trade = observation(
        primary_market.clone(),
        SourceProgram::Pump,
        70_000,
        70_000,
        "qualification-primary-at-close",
        ObservationPayload::Trade {
            side: TradeSide::Buy,
            wallet: "wallet-close-boundary".to_owned(),
            base_amount_units: 1,
            quote_amount_units: 1,
            base_reserve_units: None,
            quote_reserve_units: None,
        },
    );
    assert!(matches!(
        database
            .insert_window_observation(primary_window.window_id, &close_boundary_trade)
            .await,
        Err(PersistenceError::DiscoveryWindowObservationMismatch {
            window_id,
            field: "received_time_unix_ms",
        }) if window_id == primary_window.window_id
    ));

    let mut first_trade = None;
    for (slot, signature, side, wallet, base, quote) in [
        (
            20_000,
            "qualification-primary-buy",
            TradeSide::Buy,
            "wallet-primary-a",
            10,
            20,
        ),
        (
            30_000,
            "qualification-primary-sell",
            TradeSide::Sell,
            "wallet-primary-b",
            5,
            10,
        ),
    ] {
        let received_time_unix_ms = if first_trade.is_none() {
            i64::try_from(slot).expect("test slot fits i64") + 100
        } else {
            i64::try_from(slot).expect("test slot fits i64")
        };
        let trade = observation(
            primary_market.clone(),
            SourceProgram::Pump,
            slot,
            received_time_unix_ms,
            signature,
            ObservationPayload::Trade {
                side,
                wallet: wallet.to_owned(),
                base_amount_units: base,
                quote_amount_units: quote,
                base_reserve_units: None,
                quote_reserve_units: None,
            },
        );
        assert!(
            database
                .insert_window_observation(primary_window.window_id, &trade)
                .await
                .expect("insert primary member")
        );
        if first_trade.is_none() {
            first_trade = Some(trade);
        }
    }
    let mut earlier_trade_replay = first_trade.expect("first trade fixture");
    earlier_trade_replay.received_time_unix_ms = 20_000;
    earlier_trade_replay.commitment = Commitment::Finalized;
    earlier_trade_replay.key.coordinate.transaction_index = None;
    assert!(
        !database
            .insert_window_observation(primary_window.window_id, &earlier_trade_replay)
            .await
            .expect("earlier compatible activity replay")
    );
    let replayed_trade_times = sqlx::query_as::<_, (i64, i64)>(
        "SELECT observation.received_time_unix_ms, member.admitted_at_unix_ms \
         FROM chain_observations AS observation \
         JOIN discovery_window_observations AS member \
           ON member.observation_id = observation.id \
         WHERE member.window_id = $1 \
           AND observation.signature = 'qualification-primary-buy'",
    )
    .bind(primary_window.window_id)
    .fetch_one(&verification_pool)
    .await
    .expect("earliest activity receipt after reordered replay");
    assert_eq!(replayed_trade_times, (20_000, 20_000));

    let mut later_trade_replay = earlier_trade_replay;
    later_trade_replay.received_time_unix_ms = 70_001;
    later_trade_replay.commitment = Commitment::Finalized;
    later_trade_replay.key.coordinate.transaction_index = None;
    assert!(
        !database
            .insert_window_observation(primary_window.window_id, &later_trade_replay)
            .await
            .expect("already-admitted transport replay remains a no-op after the close boundary")
    );
    let replay_memberships = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM discovery_window_observations WHERE window_id = $1",
    )
    .bind(primary_window.window_id)
    .fetch_one(&verification_pool)
    .await
    .expect("membership count after activity replay");
    let replay_work = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) \
         FROM observation_work AS work \
         JOIN chain_observations AS observation ON observation.id = work.observation_id \
         WHERE observation.mint = 'qualification-mint-primary'",
    )
    .fetch_one(&verification_pool)
    .await
    .expect("work count after activity replay");
    assert_eq!(replay_memberships, 3);
    assert_eq!(replay_work, 3);

    assert_eq!(
        project_all_pending(&database, "qualification-primary-worker").await,
        3,
        "only the opening fact and two half-open members should be queued"
    );

    let observed_primary = database
        .load_discovery_token(&opening.market.mint)
        .await
        .expect("load observed primary token")
        .expect("primary token projected");
    let mut premature_approval = observed_primary;
    premature_approval.stage = DiscoveryStage::Approved;
    assert!(matches!(
        database
            .commit_discovery_approval(
                &premature_approval,
                "QUALIFICATION_INTEGRATION",
                "before-pass",
            )
            .await,
        Err(PersistenceError::CandidateNotQualified { .. })
    ));

    let now_unix_ms = i64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after Unix epoch")
            .as_millis(),
    )
    .expect("current timestamp fits i64");
    let retained_while_active = database
        .prune_retained_history(RetentionPolicy {
            terminal_history_before_unix_ms: now_unix_ms + 60_000,
            projection_events_before_unix_ms: now_unix_ms + 60_000,
            quarantine_before_unix_ms: now_unix_ms + 60_000,
            batch_size: 10_000,
        })
        .await
        .expect("active-window retention prune");
    assert_eq!(
        retained_while_active.terminal_observations, 0,
        "terminal work remains protected while its qualification window is active"
    );
    let primary_evidence = database
        .load_discovery_window(primary_window.window_id)
        .await
        .expect("active window evidence survives retention");
    assert_eq!(primary_evidence.observations.len(), 3);

    let primary_finalization =
        pass_finalization(&primary_evidence, 70_000, "primary-complete-pass");
    let mut changed_evidence_count = primary_finalization.clone();
    changed_evidence_count.observation_count += 1;
    assert!(matches!(
        database
            .finalize_discovery_window(&changed_evidence_count)
            .await,
        Err(PersistenceError::DiscoveryWindowEvidenceChanged { window_id })
            if window_id == primary_window.window_id
    ));
    assert!(
        database
            .load_active_discovery_window_ids(None)
            .await
            .expect("active windows after evidence mismatch")
            .contains(&primary_window.window_id),
        "an evidence-count race must leave the window retryable"
    );
    assert_eq!(
        database
            .load_discovery_snapshot()
            .await
            .expect("counters after evidence mismatch")
            .counters
            .qualification_pending,
        1
    );
    let mut fabricated_pass = primary_finalization.clone();
    fabricated_pass.snapshot = serde_json::json!({
        "fabricated": true,
        "observation_count": fabricated_pass.observation_count,
    });
    assert!(matches!(
        database.finalize_discovery_window(&fabricated_pass).await,
        Err(PersistenceError::DiscoveryWindowFinalizationEvidenceMismatch { window_id })
            if window_id == primary_window.window_id
    ));

    assert!(
        database
            .finalize_discovery_window(&primary_finalization)
            .await
            .expect("finalize primary PASS")
    );
    assert!(
        !database
            .finalize_discovery_window(&primary_finalization)
            .await
            .expect("identical finalization is idempotent")
    );
    let mut conflicting_finalization = primary_finalization.clone();
    conflicting_finalization.snapshot =
        serde_json::json!({"fixture": "conflicting-final-evidence"});
    assert!(matches!(
        database
            .finalize_discovery_window(&conflicting_finalization)
            .await,
        Err(PersistenceError::DiscoveryWindowFinalizationConflict { window_id })
            if window_id == primary_window.window_id
    ));

    let qualified_primary = database
        .load_discovery_token(&opening.market.mint)
        .await
        .expect("load qualified primary token")
        .expect("qualified primary token retained");
    assert_eq!(qualified_primary.stage, DiscoveryStage::Qualified);
    let primary_summary = qualified_primary
        .qualification
        .as_ref()
        .expect("qualified token retains window summary");
    assert_eq!(primary_summary.decision, QualificationDecision::Pass);
    assert_eq!(primary_summary.completeness, WindowCompleteness::Complete);
    assert_eq!(primary_summary.ruleset_revision, qualification_v1.revision);
    let counters_after_pass = database
        .load_discovery_snapshot()
        .await
        .expect("counters after primary PASS")
        .counters;
    assert_eq!(counters_after_pass.qualification_pending, 0);
    assert_eq!(counters_after_pass.qualified, 1);

    let mut approved_primary = qualified_primary;
    approved_primary.stage = DiscoveryStage::Approved;
    assert_eq!(
        database
            .commit_discovery_approval(
                &approved_primary,
                "QUALIFICATION_INTEGRATION",
                "complete-pass",
            )
            .await
            .expect("approve complete PASS"),
        DiscoveryProjectionMutation::Promoted
    );

    let superseded_pump_market = market(
        "qualification-mint-superseded",
        "qualification-bonding-curve-superseded",
        Venue::PumpBondingCurve,
        Some(QUOTE_MINT),
    );
    let superseded_opening = observation(
        superseded_pump_market.clone(),
        SourceProgram::Pump,
        100_000,
        100_000,
        "qualification-superseded-create",
        ObservationPayload::TokenCreated {
            name: "Qualification Superseded".to_owned(),
            symbol: "QSUP".to_owned(),
            uri: "https://example.invalid/qualification-superseded.json".to_owned(),
            creator: "creator-superseded".to_owned(),
            user: "user-superseded".to_owned(),
        },
    );
    let superseded_window = database
        .open_discovery_window(
            &superseded_opening,
            &new_window(
                &superseded_opening,
                160_000,
                "qualification-run-superseded",
                prefilter,
                qualification_v2,
            ),
        )
        .await
        .expect("open superseded Pump window");
    for (slot, signature, side, wallet, base, quote) in [
        (
            110_000,
            "qualification-superseded-buy",
            TradeSide::Buy,
            "wallet-superseded-a",
            10,
            20,
        ),
        (
            120_000,
            "qualification-superseded-sell",
            TradeSide::Sell,
            "wallet-superseded-b",
            5,
            10,
        ),
    ] {
        let trade = observation(
            superseded_pump_market.clone(),
            SourceProgram::Pump,
            slot,
            i64::try_from(slot).expect("test slot fits i64"),
            signature,
            ObservationPayload::Trade {
                side,
                wallet: wallet.to_owned(),
                base_amount_units: base,
                quote_amount_units: quote,
                base_reserve_units: None,
                quote_reserve_units: None,
            },
        );
        assert!(
            database
                .insert_window_observation(superseded_window.window_id, &trade)
                .await
                .expect("insert superseded Pump member")
        );
    }
    assert_eq!(
        project_all_pending(&database, "qualification-superseded-pump-worker").await,
        3
    );

    let pump_swap_trade = observation(
        market(
            &superseded_opening.market.mint,
            "qualification-pump-swap-pool",
            Venue::PumpSwap,
            Some(QUOTE_MINT),
        ),
        SourceProgram::PumpSwap,
        170_000,
        170_000,
        "qualification-superseding-pump-swap-buy",
        ObservationPayload::Trade {
            side: TradeSide::Buy,
            wallet: "wallet-pump-swap".to_owned(),
            base_amount_units: 100,
            quote_amount_units: 200,
            base_reserve_units: None,
            quote_reserve_units: None,
        },
    );
    assert!(
        database
            .insert_observation(&pump_swap_trade)
            .await
            .expect("insert superseding PumpSwap fact")
    );
    assert_eq!(
        project_all_pending(&database, "qualification-superseding-swap-worker").await,
        1
    );
    let projection_before_stale_finalization = database
        .load_discovery_token(&superseded_opening.market.mint)
        .await
        .expect("load superseding projection")
        .expect("superseding projection retained");
    assert_eq!(
        projection_before_stale_finalization.primary_venue,
        Venue::PumpSwap
    );
    assert_eq!(
        projection_before_stale_finalization.stage,
        DiscoveryStage::Observed
    );

    let superseded_evidence = database
        .load_discovery_window(superseded_window.window_id)
        .await
        .expect("load superseded Pump evidence");
    assert_eq!(
        superseded_evidence.qualification_revision,
        qualification_v2.revision
    );
    let superseded_finalization =
        pass_finalization(&superseded_evidence, 170_000, "superseded-pump-pass");
    assert!(
        database
            .finalize_discovery_window(&superseded_finalization)
            .await
            .expect("finalize superseded Pump audit")
    );
    let projection_after_stale_finalization = database
        .load_discovery_token(&superseded_opening.market.mint)
        .await
        .expect("load projection after stale finalization")
        .expect("superseding projection retained after audit");
    assert_eq!(
        projection_after_stale_finalization, projection_before_stale_finalization,
        "a stale Pump assessment must not mutate the current PumpSwap candidate"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM discovery_window_snapshots WHERE window_id = $1",
        )
        .bind(superseded_window.window_id)
        .fetch_one(&verification_pool)
        .await
        .expect("superseded snapshot audit count"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM qualification_assessments WHERE window_id = $1",
        )
        .bind(superseded_window.window_id)
        .fetch_one(&verification_pool)
        .await
        .expect("superseded assessment audit count"),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM discovery_windows WHERE id = $1")
            .bind(superseded_window.window_id)
            .fetch_one(&verification_pool)
            .await
            .expect("superseded durable window status"),
        "FINALIZED"
    );

    let counters_before_rebuild = database
        .load_discovery_snapshot()
        .await
        .expect("counters before rebuild")
        .counters;
    assert_eq!(counters_before_rebuild.observed, 2);
    assert_eq!(counters_before_rebuild.pending, 0);
    assert_eq!(counters_before_rebuild.qualified, 1);
    assert_eq!(counters_before_rebuild.qualification_pending, 0);
    assert_eq!(counters_before_rebuild.approved, 1);
    database
        .rebuild_discovery_projection()
        .await
        .expect("rebuild qualification counters");
    let counters_after_rebuild = database
        .load_discovery_snapshot()
        .await
        .expect("counters after rebuild")
        .counters;
    assert_eq!(
        counters_after_rebuild, counters_before_rebuild,
        "rebuilt counters must agree with the incremental durable projection"
    );

    database.close().await;
    verification_pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&administration_pool)
        .await
        .expect("isolated qualification schema cleanup");
    administration_pool.close().await;
}

#[tokio::test]
async fn newer_window_commit_prevents_an_in_flight_older_pass_projection() {
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
        "soldisco_mint_serialization_{}_{}",
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
        .expect("isolated mint-serialization schema");

    let base_options = PgConnectOptions::from_str(&database_url)
        .expect("valid PostgreSQL test URL")
        .options([("search_path", schema.as_str())]);
    let verification_pool = PgPoolOptions::new()
        .max_connections(4)
        .connect_with(base_options.clone())
        .await
        .expect("mint-serialization verification connection");
    let database = Database::connect_with_options(base_options.clone(), 5)
        .await
        .expect("mint-serialization database connection");
    database.migrate().await.expect("migrations");
    database
        .set_discovery_mode(DiscoveryMode::ObserveAll)
        .await
        .expect("observe-all integration projection");
    database
        .bind_network(Network::SolanaMainnet)
        .await
        .expect("mainnet database binding");

    let prefilter = database
        .initialize_prefilter_defaults(PrefilterDefaults {
            max_event_age_ms: 15_000,
            observation_window_ms: 60_000,
            max_active_windows: 128,
            rpc_requests_per_second: 8,
            rpc_max_in_flight: 16,
            rpc_request_timeout_ms: 5_000,
            rpc_rate_limit_cooldown_ms: 5_000,
        })
        .await
        .expect("initial durable prefilter defaults");
    let qualification = database
        .initialize_qualification_defaults(QualificationDefaults {
            minimum_trades: 2,
            minimum_unique_traders: 2,
            minimum_buys: 1,
            minimum_sells: 1,
            minimum_native_quote_volume_units: 1,
            minimum_stable_quote_volume_units: 1,
            maximum_single_wallet_quote_share_bps: 10_000,
        })
        .await
        .expect("initial durable qualification defaults");

    let shared_market = market(
        "qualification-mint-serialized",
        "qualification-bonding-curve-serialized",
        Venue::PumpBondingCurve,
        Some(QUOTE_MINT),
    );
    let older_opening = observation(
        shared_market.clone(),
        SourceProgram::Pump,
        10_000,
        10_000,
        "qualification-serialized-older-create",
        ObservationPayload::TokenCreated {
            name: "Qualification Serialized".to_owned(),
            symbol: "QSER".to_owned(),
            uri: "https://example.invalid/qualification-serialized.json".to_owned(),
            creator: "creator-serialized".to_owned(),
            user: "user-serialized".to_owned(),
        },
    );
    let older_window = database
        .open_discovery_window(
            &older_opening,
            &new_window(
                &older_opening,
                70_000,
                "qualification-run-serialized-older",
                prefilter,
                qualification,
            ),
        )
        .await
        .expect("open older serialized window");
    for (slot, signature, side, wallet) in [
        (
            20_000,
            "qualification-serialized-older-buy",
            TradeSide::Buy,
            "wallet-serialized-a",
        ),
        (
            30_000,
            "qualification-serialized-older-sell",
            TradeSide::Sell,
            "wallet-serialized-b",
        ),
    ] {
        let trade = observation(
            shared_market.clone(),
            SourceProgram::Pump,
            slot,
            i64::try_from(slot).expect("test slot fits i64"),
            signature,
            ObservationPayload::Trade {
                side,
                wallet: wallet.to_owned(),
                base_amount_units: 10,
                quote_amount_units: 20,
                base_reserve_units: None,
                quote_reserve_units: None,
            },
        );
        assert!(
            database
                .insert_window_observation(older_window.window_id, &trade)
                .await
                .expect("insert older serialized member")
        );
    }
    assert_eq!(
        project_all_pending(&database, "qualification-serialized-worker").await,
        3
    );
    let older_evidence = database
        .load_discovery_window(older_window.window_id)
        .await
        .expect("load older serialized evidence");
    let older_pass = pass_finalization(&older_evidence, 70_000, "serialized-older-pass");

    let newer_opening = observation(
        shared_market,
        SourceProgram::Pump,
        80_000,
        80_000,
        "qualification-serialized-newer-create",
        ObservationPayload::TokenCreated {
            name: "Qualification Serialized Newer".to_owned(),
            symbol: "QSERN".to_owned(),
            uri: "https://example.invalid/qualification-serialized-newer.json".to_owned(),
            creator: "creator-serialized-newer".to_owned(),
            user: "user-serialized-newer".to_owned(),
        },
    );
    let newer_window = new_window(
        &newer_opening,
        140_000,
        "qualification-run-serialized-newer",
        prefilter,
        qualification,
    );

    // Hold the last row updated by window opening. The opener therefore owns
    // the per-mint advisory lock while its newer window remains uncommitted.
    let mut projection_blocker = verification_pool
        .begin()
        .await
        .expect("begin projection-state blocker");
    sqlx::query_scalar::<_, bool>(
        "SELECT singleton \
         FROM discovery_projection_state \
         WHERE singleton = TRUE \
         FOR UPDATE",
    )
    .fetch_one(&mut *projection_blocker)
    .await
    .expect("lock projection-state row");

    let opener_application = format!("soldisco-mint-opener-{}", std::process::id());
    let finalizer_application = format!("soldisco-mint-finalizer-{}", std::process::id());
    let opener_database = Database::connect_with_options(
        base_options.clone().application_name(&opener_application),
        1,
    )
    .await
    .expect("dedicated opener database");
    let finalizer_database = Database::connect_with_options(
        base_options
            .clone()
            .application_name(&finalizer_application),
        1,
    )
    .await
    .expect("dedicated finalizer database");

    let opener_call_database = opener_database.clone();
    let newer_opening_for_task = newer_opening.clone();
    let mut opener_task = tokio::spawn(async move {
        opener_call_database
            .open_discovery_window(&newer_opening_for_task, &newer_window)
            .await
    });
    wait_for_postgres_lock_wait(&verification_pool, &opener_application, None).await;

    let finalizer_call_database = finalizer_database.clone();
    let older_pass_for_task = older_pass.clone();
    let mut finalizer_task = tokio::spawn(async move {
        finalizer_call_database
            .finalize_discovery_window(&older_pass_for_task)
            .await
    });
    wait_for_postgres_lock_wait(&verification_pool, &finalizer_application, Some("advisory")).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(100), &mut finalizer_task)
            .await
            .is_err(),
        "the older finalizer must wait while the newer opening is uncommitted"
    );

    projection_blocker
        .commit()
        .await
        .expect("release projection-state blocker");
    let newer_opened = tokio::time::timeout(Duration::from_secs(5), &mut opener_task)
        .await
        .expect("newer opening completed after blocker release")
        .expect("newer opening task joined")
        .expect("newer window committed");
    assert!(newer_opened.window_created);
    assert_ne!(newer_opened.window_id, older_window.window_id);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), finalizer_task)
            .await
            .expect("older finalization completed after newer commit")
            .expect("older finalization task joined")
            .expect("older finalization audit committed")
    );

    let retained_projection = database
        .load_discovery_token(&newer_opening.market.mint)
        .await
        .expect("load serialized projection")
        .expect("serialized projection retained");
    assert_eq!(retained_projection.stage, DiscoveryStage::Observed);
    assert!(
        retained_projection.qualification.is_none(),
        "an older PASS must not qualify the projection after the newer window commits"
    );
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "SELECT (payload ->> 'projection_applied')::BOOLEAN \
             FROM projection_events \
             WHERE projection_name = 'DISCOVERY' \
               AND event_kind = 'QUALIFICATION_WINDOW_FINALIZED' \
               AND (payload ->> 'window_id')::BIGINT = $1 \
             ORDER BY sequence DESC \
             LIMIT 1",
        )
        .bind(older_window.window_id)
        .fetch_one(&verification_pool)
        .await
        .expect("older finalization projection marker")
    );

    opener_database.close().await;
    finalizer_database.close().await;
    database.close().await;
    verification_pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&administration_pool)
        .await
        .expect("isolated mint-serialization schema cleanup");
    administration_pool.close().await;
}
