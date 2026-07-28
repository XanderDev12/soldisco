use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use futures_util::{SinkExt as _, StreamExt as _};
use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};
use soldisco_domain::Commitment;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{Message, protocol::CloseFrame},
};

use crate::{ProgramLogNotification, RpcError};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Creates one standard Solana `logsSubscribe` WebSocket session at a time.
///
/// The server supervisor owns reconnect timing and HTTP backfill between the
/// last durable checkpoint and a replacement subscription.
#[derive(Clone)]
pub struct SolanaPubsubClient {
    endpoint: String,
    next_request_id: Arc<AtomicU64>,
}

impl SolanaPubsubClient {
    pub fn new(endpoint: &str) -> Result<Self, RpcError> {
        let url = Url::parse(endpoint).map_err(|error| {
            RpcError::InvalidRequest(format!("invalid WebSocket RPC URL: {error}"))
        })?;
        if !matches!(url.scheme(), "ws" | "wss") {
            return Err(RpcError::InvalidRequest(
                "WebSocket RPC URL must use ws or wss".to_owned(),
            ));
        }
        Ok(Self {
            endpoint: endpoint.to_owned(),
            next_request_id: Arc::new(AtomicU64::new(1)),
        })
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub async fn subscribe_program_logs(
        &self,
        program_id: &str,
        commitment: Commitment,
    ) -> Result<ProgramLogSubscription, RpcError> {
        if program_id.is_empty() {
            return Err(RpcError::InvalidRequest(
                "program id cannot be empty".to_owned(),
            ));
        }
        let (mut socket, _) = connect_async(&self.endpoint)
            .await
            .map_err(map_websocket_error)?;
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        send_json(
            &mut socket,
            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "logsSubscribe",
                "params": [
                    {"mentions": [program_id]},
                    {"commitment": commitment_name(commitment)}
                ],
            }),
        )
        .await?;
        let subscription_id = read_subscription_response(&mut socket, request_id).await?;
        Ok(ProgramLogSubscription {
            socket,
            subscription_id,
            next_request_id: self.next_request_id.clone(),
        })
    }
}

pub struct ProgramLogSubscription {
    socket: Socket,
    subscription_id: u64,
    next_request_id: Arc<AtomicU64>,
}

impl ProgramLogSubscription {
    #[must_use]
    pub const fn subscription_id(&self) -> u64 {
        self.subscription_id
    }

    pub async fn next_notification(&mut self) -> Result<ProgramLogNotification, RpcError> {
        loop {
            let text = next_text(&mut self.socket).await?;
            let value: Value = serde_json::from_str(&text)
                .map_err(|error| RpcError::InvalidResponse(error.to_string()))?;
            if let Some(error) = value.get("error") {
                return Err(parse_rpc_error(error));
            }
            if value.get("method").and_then(Value::as_str) != Some("logsNotification") {
                continue;
            }
            let notification: LogsNotification = serde_json::from_value(value)
                .map_err(|error| RpcError::InvalidResponse(error.to_string()))?;
            if notification.jsonrpc != "2.0" {
                return Err(RpcError::InvalidResponse(
                    "PubSub notification is not JSON-RPC 2.0".to_owned(),
                ));
            }
            if notification.params.subscription != self.subscription_id {
                return Err(RpcError::InvalidResponse(format!(
                    "notification subscription {} did not match {}",
                    notification.params.subscription, self.subscription_id
                )));
            }
            return Ok(ProgramLogNotification {
                subscription_id: self.subscription_id,
                slot: notification.params.result.context.slot,
                signature: notification.params.result.value.signature,
                log_messages: notification.params.result.value.logs,
                transaction_error: compact_error(notification.params.result.value.error)?,
            });
        }
    }

    pub async fn unsubscribe(mut self) -> Result<(), RpcError> {
        let request_id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        send_json(
            &mut self.socket,
            json!({
                "jsonrpc": "2.0",
                "id": request_id,
                "method": "logsUnsubscribe",
                "params": [self.subscription_id],
            }),
        )
        .await?;
        loop {
            let text = next_text(&mut self.socket).await?;
            let response: Value = serde_json::from_str(&text)
                .map_err(|error| RpcError::InvalidResponse(error.to_string()))?;
            if response.get("id").and_then(Value::as_u64) != Some(request_id) {
                continue;
            }
            if let Some(error) = response.get("error") {
                return Err(parse_rpc_error(error));
            }
            if response.get("result").and_then(Value::as_bool) != Some(true) {
                return Err(RpcError::InvalidResponse(
                    "logsUnsubscribe did not return true".to_owned(),
                ));
            }
            self.socket.close(None).await.map_err(map_websocket_error)?;
            return Ok(());
        }
    }
}

async fn send_json(socket: &mut Socket, value: Value) -> Result<(), RpcError> {
    socket
        .send(Message::Text(value.to_string().into()))
        .await
        .map_err(map_websocket_error)
}

async fn next_text(socket: &mut Socket) -> Result<String, RpcError> {
    loop {
        match socket.next().await {
            Some(Ok(Message::Text(text))) => return Ok(text.to_string()),
            Some(Ok(Message::Ping(payload))) => socket
                .send(Message::Pong(payload))
                .await
                .map_err(map_websocket_error)?,
            Some(Ok(Message::Close(frame))) => {
                return Err(RpcError::SubscriptionClosed(close_reason(frame)));
            }
            Some(Ok(Message::Binary(_))) => {
                return Err(RpcError::InvalidResponse(
                    "PubSub server sent an unexpected binary frame".to_owned(),
                ));
            }
            Some(Ok(_)) => {}
            Some(Err(error)) => return Err(map_websocket_error(error)),
            None => {
                return Err(RpcError::SubscriptionClosed(
                    "stream ended without a close frame".to_owned(),
                ));
            }
        }
    }
}

async fn read_subscription_response(socket: &mut Socket, request_id: u64) -> Result<u64, RpcError> {
    loop {
        let text = next_text(socket).await?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|error| RpcError::InvalidResponse(error.to_string()))?;
        if value.get("id").and_then(Value::as_u64) != Some(request_id) {
            continue;
        }
        if let Some(error) = value.get("error") {
            return Err(parse_rpc_error(error));
        }
        return value.get("result").and_then(Value::as_u64).ok_or_else(|| {
            RpcError::InvalidResponse("logsSubscribe response omitted numeric result".to_owned())
        });
    }
}

fn parse_rpc_error(value: &Value) -> RpcError {
    let code = value.get("code").and_then(Value::as_i64).unwrap_or(-1);
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unknown PubSub RPC error")
        .to_owned();
    if code == 429 || code == -32005 {
        RpcError::RateLimited
    } else {
        RpcError::Rpc { code, message }
    }
}

fn compact_error(error: Option<Value>) -> Result<Option<String>, RpcError> {
    error
        .map(|value| {
            serde_json::to_string(&value)
                .map_err(|error| RpcError::InvalidResponse(error.to_string()))
        })
        .transpose()
}

fn commitment_name(commitment: Commitment) -> &'static str {
    match commitment {
        Commitment::Processed => "processed",
        Commitment::Confirmed => "confirmed",
        Commitment::Finalized => "finalized",
    }
}

fn close_reason(frame: Option<CloseFrame>) -> String {
    frame.map_or_else(
        || "peer closed without a reason".to_owned(),
        |frame| format!("{}: {}", frame.code, frame.reason),
    )
}

fn map_websocket_error(error: tokio_tungstenite::tungstenite::Error) -> RpcError {
    RpcError::Transport(error.to_string())
}

#[derive(Debug, Deserialize)]
struct LogsNotification {
    jsonrpc: String,
    params: LogsNotificationParams,
}

#[derive(Debug, Deserialize)]
struct LogsNotificationParams {
    result: LogsResult,
    subscription: u64,
}

#[derive(Debug, Deserialize)]
struct LogsResult {
    context: LogsContext,
    value: LogsValue,
}

#[derive(Debug, Deserialize)]
struct LogsContext {
    slot: u64,
}

#[derive(Debug, Deserialize)]
struct LogsValue {
    signature: String,
    #[serde(rename = "err")]
    error: Option<Value>,
    logs: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::{LogsNotification, SolanaPubsubClient, compact_error};

    #[test]
    fn validates_websocket_endpoint_scheme() {
        assert!(SolanaPubsubClient::new("wss://api.example.invalid").is_ok());
        assert!(SolanaPubsubClient::new("https://api.example.invalid").is_err());
    }

    #[test]
    fn parses_standard_solana_log_notification() {
        let notification: LogsNotification = serde_json::from_str(
            r#"{
                "jsonrpc":"2.0",
                "method":"logsNotification",
                "params":{
                    "result":{
                        "context":{"slot":5208469},
                        "value":{
                            "signature":"sig",
                            "err":null,
                            "logs":["Program data: ZmFjdA=="]
                        }
                    },
                    "subscription":24040
                }
            }"#,
        )
        .unwrap();

        assert_eq!(notification.params.subscription, 24_040);
        assert_eq!(notification.params.result.context.slot, 5_208_469);
        assert_eq!(
            notification.params.result.value.logs,
            ["Program data: ZmFjdA=="]
        );
        assert_eq!(
            compact_error(notification.params.result.value.error).unwrap(),
            None
        );
    }
}
