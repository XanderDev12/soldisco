use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use soldisco_persistence::PersistenceError;
use tracing::error;

use crate::supervisor::SupervisorError;

pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetails,
}

#[derive(Serialize)]
struct ErrorDetails {
    code: &'static str,
    message: &'static str,
}

impl ApiError {
    #[must_use]
    pub const fn not_found(code: &'static str, message: &'static str) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code,
            message,
        }
    }

    fn service_unavailable(code: &'static str, message: &'static str) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            code,
            message,
        }
    }
}

impl From<PersistenceError> for ApiError {
    fn from(error: PersistenceError) -> Self {
        error!(%error, "persistence operation failed");
        Self::service_unavailable("DATABASE_UNAVAILABLE", "The local database is unavailable.")
    }
}

impl From<SupervisorError> for ApiError {
    fn from(error: SupervisorError) -> Self {
        match error {
            SupervisorError::CollectorUnavailable => Self::service_unavailable(
                "PUMP_COLLECTOR_NOT_IMPLEMENTED",
                "Pump collection is not available in this milestone.",
            ),
            SupervisorError::Persistence(error) => error.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: ErrorDetails {
                code: self.code,
                message: self.message,
            },
        };

        (self.status, Json(body)).into_response()
    }
}
