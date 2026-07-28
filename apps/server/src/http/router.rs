use axum::{
    Router,
    http::{
        HeaderValue, Method,
        header::{ACCEPT, CONTENT_TYPE},
    },
    routing::{get, post},
};
use thiserror::Error;
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, TraceLayer},
};

use crate::{
    config::Config,
    http::routes::{discovery, events, health, stream, tokens},
    state::AppState,
};

#[derive(Debug, Error)]
pub enum RouterError {
    #[error("WEB_ORIGIN is not a valid HTTP header value")]
    InvalidWebOrigin,
}

pub fn build(state: AppState, config: &Config) -> Result<Router, RouterError> {
    let web_origin =
        HeaderValue::from_str(&config.web_origin).map_err(|_| RouterError::InvalidWebOrigin)?;
    let cors = CorsLayer::new()
        .allow_origin(web_origin)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([ACCEPT, CONTENT_TYPE]);

    let routes = Router::new()
        .route("/health", get(health::get))
        .route("/stream", get(stream::get))
        .route("/stream/start", post(stream::start))
        .route("/stream/stop", post(stream::stop))
        .route("/discovery", get(discovery::get))
        .route("/tokens/{mint}", get(tokens::get))
        .route("/events", get(events::get));

    Ok(Router::new()
        .nest("/api/v1", routes)
        .layer(cors)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().include_headers(false)),
        )
        .with_state(state))
}
