use std::{
    env,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use sqlx::{
    Row as _,
    postgres::{PgConnectOptions, PgPoolOptions},
};

#[tokio::test]
async fn qualification_migration_demotes_unproven_legacy_approvals() {
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
        "soldisco_upgrade_{}_{}",
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
        .expect("isolated upgrade schema");
    let connect_options = PgConnectOptions::from_str(&database_url)
        .expect("valid PostgreSQL test URL")
        .options([("search_path", schema.as_str())]);
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(connect_options)
        .await
        .expect("upgrade test connection");

    for migration in [
        include_str!("../migrations/0001_foundation.sql"),
        include_str!("../migrations/0002_durable_discovery_pipeline.sql"),
        include_str!("../migrations/0003_persistence_hardening.sql"),
        include_str!("../migrations/0004_prefilter_defaults.sql"),
    ] {
        sqlx::raw_sql(migration)
            .execute(&pool)
            .await
            .expect("pre-qualification migration");
    }

    sqlx::query(
        "INSERT INTO database_network_binding (singleton, network) \
         VALUES (TRUE, 'SOLANA_MAINNET')",
    )
    .execute(&pool)
    .await
    .expect("network binding");
    let legacy_token = json!({
        "mint": "legacy-mint",
        "name": null,
        "symbol": null,
        "primary_venue": "PUMP_BONDING_CURVE",
        "market_address": "legacy-curve",
        "quote_mint": null,
        "source_program": "PUMP",
        "stage": "APPROVED",
        "last_event_kind": "CREATE",
        "observed_slot": 1,
        "first_observed_unix_ms": 1,
        "last_observed_unix_ms": 1,
        "latest_signature": "legacy-signature",
        "activity": {
            "trades": 0,
            "buys": 0,
            "sells": 0,
            "unique_traders": 0,
            "base_volume_units": "0",
            "quote_volume_units": "0"
        },
        "risk_score": null,
        "opportunity_score": null
    });
    sqlx::query(
        "INSERT INTO discovery_tokens (\
            mint, stage, network, source_program, venue, market_address, \
            quote_mint, event_kind, observed_slot, observed_signature, \
            instruction_index, event_index, decision_source, decision_version, token\
         ) VALUES (\
            'legacy-mint', 'APPROVED', 'SOLANA_MAINNET', 'PUMP', \
            'PUMP_BONDING_CURVE', 'legacy-curve', NULL, 'CREATE', 1, \
            'legacy-signature', 0, 0, 'LEGACY_TEST', '1', $1\
         )",
    )
    .bind(legacy_token)
    .execute(&pool)
    .await
    .expect("legacy approved token");
    sqlx::query(
        "UPDATE discovery_projection_state \
         SET observed = 1, approved = 1 \
         WHERE singleton = TRUE",
    )
    .execute(&pool)
    .await
    .expect("legacy counters");

    sqlx::raw_sql(include_str!(
        "../migrations/0005_durable_qualification_windows.sql"
    ))
    .execute(&pool)
    .await
    .expect("qualification upgrade");

    let token = sqlx::query(
        "SELECT stage, token ->> 'stage' AS json_stage, \
                token ? 'qualification' AS has_qualification, \
                token -> 'qualification' AS qualification \
         FROM discovery_tokens \
         WHERE mint = 'legacy-mint'",
    )
    .fetch_one(&pool)
    .await
    .expect("upgraded token");
    assert_eq!(token.get::<String, _>("stage"), "OBSERVED");
    assert_eq!(token.get::<String, _>("json_stage"), "OBSERVED");
    assert!(token.get::<bool, _>("has_qualification"));
    assert!(token.get::<serde_json::Value, _>("qualification").is_null());

    let state = sqlx::query(
        "SELECT mode, approved, qualified \
         FROM discovery_projection_state \
         WHERE singleton = TRUE",
    )
    .fetch_one(&pool)
    .await
    .expect("upgraded projection state");
    assert_eq!(state.get::<String, _>("mode"), "QUALIFIED_ONLY");
    assert_eq!(state.get::<i64, _>("approved"), 0);
    assert_eq!(state.get::<i64, _>("qualified"), 0);

    pool.close().await;
    sqlx::query(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .execute(&administration_pool)
        .await
        .expect("drop upgrade schema");
    administration_pool.close().await;
}
