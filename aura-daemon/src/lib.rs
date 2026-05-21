use aura_core::orchestrator::Engine;
use aura_core::task::TaskType;
use aura_core::TaskId;
use axum::{
    body::Body,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{error, info};

#[derive(RustEmbed)]
#[folder = "web/"]
struct Assets;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    pub _jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub id: Value,
}

#[derive(Debug, Deserialize)]
pub struct ExtensionAddRequest {
    pub uri: String,
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

pub struct AppState {
    pub engine: Arc<Engine>,
    pub rpc_secret: Option<String>,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/*file", get(static_handler))
        .route("/jsonrpc", post(handle_jsonrpc))
        .route("/ws", get(handle_ws))
        .route("/extension/add", post(handle_extension_add))
        .with_state(state)
}

async fn index_handler() -> impl IntoResponse {
    static_handler(Path("index.html".to_string())).await
}

async fn static_handler(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');
    let mime_type = mime_guess::from_path(path).first_or_octet_stream();

    match Assets::get(path) {
        Some(content) => Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime_type.as_ref())
            .body(Body::from(content.data))
            .unwrap(),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("404 Not Found"))
            .unwrap(),
    }
}

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

    let result = match payload.method.as_str() {
        "aria2.addUri" => handle_add_uri(&state.engine, payload.params).await,
        "aria2.tellActive" => handle_tell_active(&state.engine).await,
        "aria2.pause" => handle_pause(&state.engine, payload.params).await,
        "aria2.unpause" => handle_unpause(&state.engine, payload.params).await,
        "aria2.remove" => handle_remove(&state.engine, payload.params).await,
        "aura.getConfig" => handle_get_config(&state.engine).await,
        _ => Err(json!({ "code": -32601, "message": "Method not found" })),
    };

    match result {
        Ok(res) => (
            StatusCode::OK,
            Json(json!(JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                result: Some(res),
                error: None,
                id: payload.id,
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
            })),
        )
            .into_response(),
    }
}

pub async fn handle_ws(
    ws: WebSocketUpgrade,
    headers: HeaderMap,
    Query(query): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
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
            return (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
        }
    }

    ws.on_upgrade(move |socket| ws_session(socket, state.engine.clone()))
}

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
                if let Err(e) = sender.send(Message::Text(text)).await {
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

    let id = TaskId(rand::random());
    let name = "browser-download".to_string();
    let sources = vec![(uri, ttype)];

    match state
        .engine
        .add_task_with_sources(id, name, sources, None)
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

async fn handle_add_uri(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let params = params.ok_or_else(|| json!({ "code": -32602, "message": "Invalid params" }))?;
    let uris: Vec<String> = serde_json::from_value(params[0].clone())
        .map_err(|_| json!({ "code": -32602, "message": "Invalid URIs" }))?;

    if uris.is_empty() {
        return Err(json!({ "code": -32602, "message": "Empty URI list" }));
    }

    let id = TaskId(rand::random());
    let name = "rpc-download".to_string();
    let sources: Vec<(String, TaskType)> = uris
        .into_iter()
        .map(|u| {
            let ttype = if u.ends_with(".torrent") {
                TaskType::BitTorrent
            } else {
                TaskType::Http
            };
            (u, ttype)
        })
        .collect();

    engine
        .add_task_with_sources(id, name, sources, None)
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;

    Ok(json!(id.0.to_string()))
}

async fn handle_tell_active(engine: &Engine) -> Result<Value, Value> {
    let active = engine
        .tell_active()
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;

    let res: Vec<Value> = active
        .into_iter()
        .map(|t| {
            json!({
                "gid": t.id.0.to_string(),
                "status": format!("{:?}", t.phase).to_lowercase(),
                "totalLength": t.total_length.to_string(),
                "completedLength": t.completed_length.to_string(),
                "name": t.name,
            })
        })
        .collect();

    Ok(json!(res))
}

async fn handle_pause(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine
        .pause(gid)
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;
    Ok(json!("OK"))
}

async fn handle_unpause(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine
        .resume(gid)
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;
    Ok(json!("OK"))
}

async fn handle_remove(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine
        .remove(gid)
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;
    Ok(json!("OK"))
}

async fn handle_get_config(engine: &Engine) -> Result<Value, Value> {
    let config = engine
        .tell_config()
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;
    Ok(json!(*config))
}

fn parse_gid(params: Option<Value>) -> Result<TaskId, Value> {
    let params = params.ok_or_else(|| json!({ "code": -32602, "message": "Invalid params" }))?;
    let gid_str: String = serde_json::from_value(params[0].clone())
        .map_err(|_| json!({ "code": -32602, "message": "Invalid GID" }))?;
    let gid = gid_str
        .parse::<u64>()
        .map_err(|_| json!({ "code": -32602, "message": "Invalid GID format" }))?;
    Ok(TaskId(gid))
}
