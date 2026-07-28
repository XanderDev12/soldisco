use axum::{Json, extract::State, http::HeaderMap};
use soldisco_api_contracts::{PrefilterDefaultsResponse, UpdatePrefilterDefaultsRequest};

use crate::{
    http::{error::ApiError, routes::require_local_control},
    state::AppState,
};

pub async fn get_prefilter_defaults(
    State(state): State<AppState>,
) -> Result<Json<PrefilterDefaultsResponse>, ApiError> {
    Ok(Json(state.prefilter_defaults().await?))
}

pub async fn update_prefilter_defaults(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<UpdatePrefilterDefaultsRequest>,
) -> Result<Json<PrefilterDefaultsResponse>, ApiError> {
    require_local_control(&state, &headers)?;
    Ok(Json(
        state
            .update_prefilter_defaults(request.expected_revision, request.values)
            .await?,
    ))
}
