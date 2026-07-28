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

    #[must_use]
    pub const fn forbidden(code: &'static str, message: &'static str) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code,
            message,
        }
    }

    const fn bad_request(code: &'static str, message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message,
        }
    }

    const fn conflict(code: &'static str, message: &'static str) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message,
        }
    }
}

impl From<PersistenceError> for ApiError {
    fn from(error: PersistenceError) -> Self {
        match error {
            PersistenceError::InvalidPrefilterDefaults(_)
            | PersistenceError::MustBePositive {
                field: "prefilter expected_revision",
            }
            | PersistenceError::ValueOutOfRange {
                field: "prefilter expected_revision",
            } => Self::bad_request(
                "INVALID_PREFILTER_DEFAULTS",
                "The submitted prefilter defaults are invalid.",
            ),
            PersistenceError::PrefilterDefaultsRevisionConflict { .. } => Self::conflict(
                "PREFILTER_DEFAULTS_REVISION_CONFLICT",
                "The prefilter defaults changed after this form was loaded. Refresh and try again.",
            ),
            error => {
                error!(%error, "persistence operation failed");
                Self::service_unavailable(
                    "DATABASE_UNAVAILABLE",
                    "The local database is unavailable.",
                )
            }
        }
    }
}

impl From<SupervisorError> for ApiError {
    fn from(error: SupervisorError) -> Self {
        match error {
            SupervisorError::ShuttingDown => Self::service_unavailable(
                "STREAM_SHUTTING_DOWN",
                "The discovery stream is shutting down.",
            ),
            SupervisorError::PrefilterSettingsRequireStopped => Self::conflict(
                "STREAM_MUST_BE_STOPPED",
                "Stop the discovery stream before changing prefilter defaults.",
            ),
            SupervisorError::Pipeline(error) => {
                error!(%error, "discovery pipeline failed to start");
                Self::service_unavailable(
                    "STREAM_START_FAILED",
                    "The discovery stream could not connect to its configured sources.",
                )
            }
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

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use soldisco_api_contracts::{IntegerBounds, PrefilterDefaultsValidationError};
    use soldisco_persistence::PersistenceError;

    use super::ApiError;
    use crate::supervisor::SupervisorError;

    #[test]
    fn invalid_and_stale_prefilter_updates_are_client_errors() {
        let invalid = ApiError::from(PersistenceError::InvalidPrefilterDefaults(
            PrefilterDefaultsValidationError::OutOfBounds {
                field: "rpc_max_in_flight",
                value: 0,
                bounds: IntegerBounds {
                    minimum: 1,
                    maximum: 128,
                },
            },
        ));
        assert_eq!(invalid.status, StatusCode::BAD_REQUEST);
        assert_eq!(invalid.code, "INVALID_PREFILTER_DEFAULTS");

        let stale = ApiError::from(PersistenceError::PrefilterDefaultsRevisionConflict {
            expected: 1,
            actual: Some(2),
        });
        assert_eq!(stale.status, StatusCode::CONFLICT);
        assert_eq!(stale.code, "PREFILTER_DEFAULTS_REVISION_CONFLICT");
    }

    #[test]
    fn running_stream_settings_update_is_an_explicit_conflict() {
        let error = ApiError::from(SupervisorError::PrefilterSettingsRequireStopped);

        assert_eq!(error.status, StatusCode::CONFLICT);
        assert_eq!(error.code, "STREAM_MUST_BE_STOPPED");
    }
}
