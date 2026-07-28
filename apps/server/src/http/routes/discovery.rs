use axum::{Json, extract::State};
use soldisco_api_contracts::DiscoverySnapshot;

use crate::state::AppState;

pub async fn get(State(state): State<AppState>) -> Json<DiscoverySnapshot> {
    Json(state.discovery_snapshot().await)
}
