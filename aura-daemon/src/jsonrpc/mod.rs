use super::types::{AppState, JsonRpcRequest, JsonRpcResponse};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

mod download;
pub mod router;
mod system;
pub mod utils;

pub use download::*;
pub use router::RpcRouter;
pub use system::*;

pub fn authenticate(
    headers: &HeaderMap,
    secret: &Option<String>,
) -> Result<(), (StatusCode, Json<Value>)> {
    if let Some(expected_secret) = secret {
        let auth_header = headers
            .get(aura_core::RPC_AUTH_HEADER)
            .or_else(|| headers.get("Authorization"));

        let is_valid = match auth_header {
            Some(val) => {
                let val_str = val.to_str().unwrap_or("");
                val_str == expected_secret || val_str == format!("Bearer {}", expected_secret)
            }
            None => false,
        };

        if !is_valid {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(
                    json!({ "error": format!("Unauthorized. Invalid or missing {}.", aura_core::RPC_AUTH_HEADER) }),
                ),
            ));
        }
    }
    Ok(())
}

pub async fn handle_jsonrpc(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<JsonRpcRequest>,
) -> impl IntoResponse {
    if let Err(err) = authenticate(&headers, &state.rpc_secret) {
        return err.into_response();
    }

    info!("RPC Method: {}", payload.method);

    let router = RpcRouter::new(state.engine.clone());
    let (result, already_exists) = router.route(payload.method.as_str(), payload.params).await;

    match result {
        Ok(res) => (
            StatusCode::OK,
            Json(json!(JsonRpcResponse {
                jsonrpc: aura_core::JSONRPC_VERSION.to_string(),
                result: Some(res),
                error: None,
                id: payload.id,
                already_exists,
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::OK,
            Json(json!(JsonRpcResponse {
                jsonrpc: aura_core::JSONRPC_VERSION.to_string(),
                result: None,
                error: Some(err),
                id: payload.id,
                already_exists: None,
            })),
        )
            .into_response(),
    }
}

#[cfg(test)]
#[path = "../jsonrpc_tests.rs"]
mod tests;
