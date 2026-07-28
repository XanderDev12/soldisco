use axum::{Json, extract::State, http::HeaderMap};
use soldisco_api_contracts::{StreamCommandResponse, StreamStateResponse};

use crate::{http::error::ApiError, state::AppState};

pub async fn get(State(state): State<AppState>) -> Result<Json<StreamStateResponse>, ApiError> {
    Ok(Json(state.supervisor().state().await?))
}

pub async fn start(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<StreamCommandResponse>, ApiError> {
    require_local_control(&state, &headers)?;
    Ok(Json(state.supervisor().start().await?))
}

pub async fn stop(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<StreamCommandResponse>, ApiError> {
    require_local_control(&state, &headers)?;
    Ok(Json(state.supervisor().stop().await?))
}

fn require_local_control(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if state.local_control_authorized(headers) {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "LOCAL_CONTROL_FORBIDDEN",
            "This local stream-control request was not authorized.",
        ))
    }
}
