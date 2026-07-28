use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use soldisco_api_contracts::HealthResponse;

use crate::state::AppState;

pub async fn get(State(state): State<AppState>) -> Response {
    let stream = state.supervisor().status().await;
    match state.database().health().await {
        Ok(()) => {
            let overall = if matches!(
                stream,
                soldisco_api_contracts::StreamStatus::Degraded
                    | soldisco_api_contracts::StreamStatus::Error
            ) {
                "DEGRADED"
            } else {
                "UP"
            };
            (
                StatusCode::OK,
                Json(HealthResponse {
                    status: overall.to_owned(),
                    database: "UP".to_owned(),
                    stream,
                }),
            )
                .into_response()
        }
        Err(error) => {
            tracing::error!(%error, "health check could not reach PostgreSQL");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthResponse {
                    status: "DEGRADED".to_owned(),
                    database: "DOWN".to_owned(),
                    stream,
                }),
            )
                .into_response()
        }
    }
}
