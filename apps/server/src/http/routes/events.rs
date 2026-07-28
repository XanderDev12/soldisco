use std::{convert::Infallible, time::Duration};

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use tokio_stream::{Stream, StreamExt, wrappers::BroadcastStream};

use crate::state::AppState;

pub async fn get(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let initial = event_from(state.resync_event());
    let receiver = state.subscribe();
    let shutdown = state.shutdown_token();
    let live = BroadcastStream::new(receiver).filter_map(move |result| {
        let envelope = match result {
            Ok(envelope) => envelope,
            Err(_) => state.resync_event(),
        };
        Some(Ok(event_from(envelope)))
    });
    let stream = futures_util::StreamExt::take_until(
        tokio_stream::once(Ok(initial)).chain(live),
        shutdown.cancelled_owned(),
    );

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}

fn event_from(envelope: soldisco_api_contracts::LiveEnvelope) -> Event {
    Event::default()
        .id(envelope.sequence.to_string())
        .event("soldisco")
        .json_data(envelope)
        .unwrap_or_else(|_| {
            Event::default()
                .event("soldisco")
                .data(r#"{"sequence":0,"event":{"type":"RESYNC_REQUIRED"}}"#)
        })
}
