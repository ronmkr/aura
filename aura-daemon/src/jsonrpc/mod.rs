use super::types::{AppState, JsonRpcRequest, JsonRpcResponse};
use crate::jsonrpc::utils::rpc_error;
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
mod system;
pub mod utils;

pub use download::*;
pub use system::*;

pub fn authenticate(
    headers: &HeaderMap,
    secret: &Option<String>,
) -> Result<(), (StatusCode, Json<Value>)> {
    if let Some(expected_secret) = secret {
        let auth_header = headers
            .get("X-Aura-Token")
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
                Json(json!({ "error": "Unauthorized. Invalid or missing X-Aura-Token." })),
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

    let (result, already_exists) = match payload.method.as_str() {
        "aura.addUri" => match handle_add_uri(&state.engine, payload.params).await {
            Ok((val, exists)) => (Ok(val), exists),
            Err(err) => (Err(err), None),
        },
        _ => {
            let res = match payload.method.as_str() {
                "aura.tellActive" => handle_tell_active(&state.engine).await,
                "aura.pause" => handle_pause(&state.engine, payload.params).await,
                "aura.unpause" => handle_unpause(&state.engine, payload.params).await,
                "aura.remove" => handle_remove(&state.engine, payload.params).await,
                "aura.changeOption" => handle_change_option(&state.engine, payload.params).await,
                "aura.refreshUri" => handle_refresh(&state.engine, payload.params).await,
                "aura.getConfig" => handle_get_config(&state.engine).await,
                "aura.getVersion" => handle_get_version().await,
                "aura.getSessionInfo" => handle_get_session_info().await,
                "aura.tellStopped" => handle_tell_stopped(&state.engine, payload.params).await,
                "aura.tellWaiting" => handle_tell_waiting(&state.engine, payload.params).await,
                "aura.getStatus" => handle_get_status(&state.engine, payload.params).await,
                "aura.purgeDownloadResult" => handle_purge_download_result(&state.engine).await,
                "aura.removeDownloadResult" => {
                    handle_remove_download_result(&state.engine, payload.params).await
                }
                "aura.saveSession" => handle_save_session().await,
                "aura.shutdown" => handle_shutdown(&state.engine).await,
                "aura.forceShutdown" => handle_shutdown(&state.engine).await,
                "aura.changeGlobalOption" => {
                    handle_change_global_option(&state.engine, payload.params).await
                }
                "aura.getGlobalStat" => handle_get_global_stat(&state.engine).await,
                "aura.getFiles" => handle_get_files(&state.engine, payload.params).await,
                "aura.setFileSelection" => {
                    handle_set_file_selection(&state.engine, payload.params).await
                }
                "aura.addFromFolder" => handle_add_from_folder(&state.engine, payload.params).await,
                "aura.addFromFile" => handle_add_from_file(&state.engine, payload.params).await,
                _ => Err(rpc_error(-32601, "Method not found")),
            };
            (res, None)
        }
    };

    match result {
        Ok(res) => (
            StatusCode::OK,
            Json(json!(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
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
                jsonrpc: "2.0".to_string(),
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
