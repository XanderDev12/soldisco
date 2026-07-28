use axum::http::HeaderMap;

use crate::{http::error::ApiError, state::AppState};

pub mod discovery;
pub mod events;
pub mod health;
pub mod settings;
pub mod stream;
pub mod tokens;

pub(super) fn require_local_control(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    if state.local_control_authorized(headers) {
        Ok(())
    } else {
        Err(ApiError::forbidden(
            "LOCAL_CONTROL_FORBIDDEN",
            "This local control request was not authorized.",
        ))
    }
}
