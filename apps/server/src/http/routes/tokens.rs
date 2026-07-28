use axum::{
    Json,
    extract::{Path, State},
};
use soldisco_api_contracts::ApprovedToken;

use crate::{http::error::ApiError, state::AppState};

pub async fn get(
    State(state): State<AppState>,
    Path(mint): Path<String>,
) -> Result<Json<ApprovedToken>, ApiError> {
    state.approved_token(&mint).await.map(Json).ok_or_else(|| {
        ApiError::not_found(
            "TOKEN_NOT_FOUND",
            "No approved discovery record exists for this mint.",
        )
    })
}
