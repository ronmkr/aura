use super::jsonrpc::authenticate;
use super::types::{AppState, ExtensionAddRequest};
use aura_core::task::TaskType;
use aura_core::TaskId;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use std::sync::Arc;
use tracing::info;

pub async fn handle_extension_add(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<ExtensionAddRequest>,
) -> impl IntoResponse {
    if let Err(err) = authenticate(&headers, &state.rpc_secret) {
        return err.into_response();
    }

    info!("Extension Add: {}", payload.uri);

    let uri = payload.uri;
    let ttype = if uri.starts_with("magnet:")
        || uri.ends_with(".torrent")
        || payload.mime_type.as_deref() == Some("application/x-bittorrent")
    {
        TaskType::BitTorrent
    } else {
        TaskType::Http
    };

    let id = TaskId::random();
    let name = "browser-download".to_string();
    let sources = vec![(uri, ttype)];

    match state
        .engine
        .add_task_with_sources(id, None, name, sources, None)
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(json!({ "success": true, "gid": id.0.to_string() })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": e.to_string() })),
        )
            .into_response(),
    }
}
