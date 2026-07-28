use axum::{
    Json,
    extract::{Path, State},
};
use soldisco_api_contracts::DiscoveryToken;

use crate::{http::error::ApiError, state::AppState};

pub async fn get(
    State(state): State<AppState>,
    Path(mint): Path<String>,
) -> Result<Json<DiscoveryToken>, ApiError> {
    state
        .discovery_token(&mint)
        .await?
        .map(Json)
        .ok_or_else(|| {
            ApiError::not_found(
                "TOKEN_NOT_FOUND",
                "No discovery record exists for this mint.",
            )
        })
}
