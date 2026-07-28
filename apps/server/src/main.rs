mod config;
mod http;
pub mod jobs;
mod state;
mod supervisor;

use std::error::Error;

use config::Config;
use soldisco_persistence::Database;
use state::AppState;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    dotenvy::dotenv().ok();
    let config = Config::load()?;

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&config.log_level)?)
        .json()
        .init();

    let database = Database::connect(&config.database_url, config.database_max_connections).await?;
    database.migrate().await?;
    if database.bound_network().await?.is_some() {
        database.bind_network(config.solana_network).await?;
    }
    database.rebuild_discovery_projection().await?;

    let state = AppState::new(database, &config).await?;
    let app = http::router::build(state.clone(), &config)?;
    let listener = tokio::net::TcpListener::bind(config.api_address).await?;

    info!(
        app_env = %config.app_env,
        address = %config.api_address,
        web_origin = %config.web_origin,
        "Soldisco server listening"
    );

    let shutdown_state = state.clone();
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            shutdown_state.shutdown();
        })
        .await?;

    info!("Soldisco server stopped");
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::error!(%error, "failed to install shutdown signal handler");
    }
}
