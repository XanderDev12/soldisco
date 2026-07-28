use axum::{Json, extract::State};
use soldisco_api_contracts::{StreamCommandResponse, StreamStateResponse};

use crate::{http::error::ApiError, state::AppState};

pub async fn get(State(state): State<AppState>) -> Result<Json<StreamStateResponse>, ApiError> {
    Ok(Json(state.supervisor().state().await?))
}

pub async fn start(State(state): State<AppState>) -> Result<Json<StreamCommandResponse>, ApiError> {
    Ok(Json(state.supervisor().start().await?))
}

pub async fn stop(State(state): State<AppState>) -> Result<Json<StreamCommandResponse>, ApiError> {
    Ok(Json(state.supervisor().stop().await?))
}
