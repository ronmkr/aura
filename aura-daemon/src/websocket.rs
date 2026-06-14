//! Websocket session handler streaming real-time telemetry events from the
//! internal event bus as specified by [Decision-0004](aura-docs/manual/src/decisions/0004-telemetry-and-event-bus.md).

use super::types::{AppState, WsQuery};
use aura_core::orchestrator::{Engine, EventSubscriber};
use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tracing::{error, info};

pub async fn ws_auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<WsQuery>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    let token = query
        .token
        .or_else(|| {
            headers
                .get("X-Aura-Token")
                .and_then(|v| v.to_str().ok().map(|s| s.to_string()))
        })
        .or_else(|| {
            headers
                .get("Authorization")
                .and_then(|v| v.to_str().ok().map(|s| s.replace("Bearer ", "")))
        });

    if let Some(ref expected_secret) = state.rpc_secret {
        if token.as_deref() != Some(expected_secret) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    Ok(next.run(req).await)
}

pub async fn handle_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_session(socket, state.engine.clone()))
}

/// Handles the WebSocket session lifecycle, upgrading the connection to stream
/// real-time telemetry events from the core event bus as specified in [Decision-0004](aura-docs/manual/src/decisions/0004-telemetry-and-event-bus.md).
pub async fn ws_session(socket: WebSocket, engine: Arc<Engine>) {
    let (mut sender, mut receiver) = socket.split();
    let mut event_rx = engine.subscribe();

    info!("WebSocket connection established");

    let mut send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            let msg = json!({
                "jsonrpc": "2.0",
                "method": "aura.onEvent",
                "params": event,
            });

            if let Ok(text) = serde_json::to_string(&msg) {
                if let Err(e) = sender.send(Message::Text(text.into())).await {
                    error!("Failed to send WS message: {}", e);
                    break;
                }
            }
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Close(_) = msg {
                break;
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => recv_task.abort(),
        _ = (&mut recv_task) => send_task.abort(),
    };

    info!("WebSocket connection closed");
}

#[cfg(test)]
#[path = "websocket_tests.rs"]
mod tests;
