use axum::{Json, extract::State};
use soldisco_api_contracts::DiscoverySnapshot;

use crate::{http::error::ApiError, state::AppState};

pub async fn get(State(state): State<AppState>) -> Result<Json<DiscoverySnapshot>, ApiError> {
    Ok(Json(state.discovery_snapshot().await?))
}
