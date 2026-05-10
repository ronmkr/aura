use aura_core::orchestrator::Engine;
use aura_core::task::TaskType;
use aura_core::TaskId;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: String,
    method: String,
    params: Option<Value>,
    id: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    result: Option<Value>,
    error: Option<Value>,
    id: Value,
}

#[derive(Debug, Deserialize)]
struct ExtensionAddRequest {
    uri: String,
    mime_type: Option<String>,
}

struct AppState {
    engine: Arc<Engine>,
    rpc_secret: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Starting Aura Daemon");

    // Bootstrap the engine
    let config = aura_core::Config::from_file("Aura.toml").unwrap_or_default();
    let rpc_secret = config.network.rpc_secret.clone();

    let (engine, orchestrator, storage) = match Engine::new(config).await {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Failed to initialize Aura Engine: {}", e);
            std::process::exit(1);
        }
    };
    let engine = Arc::new(engine);

    // Spawn the actors
    tokio::spawn(async move {
        if let Err(e) = orchestrator.run().await {
            eprintln!("Orchestrator error: {}", e);
        }
    });

    tokio::spawn(async move {
        if let Err(e) = storage.run().await {
            eprintln!("Storage Engine error: {}", e);
        }
    });

    let state = Arc::new(AppState { engine, rpc_secret });

    let app = Router::new()
        .route("/jsonrpc", post(handle_jsonrpc))
        .route("/extension/add", post(handle_extension_add))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6800").await?;
    info!("RPC Server listening on http://0.0.0.0:6800");
    axum::serve(listener, app).await?;

    Ok(())
}

fn authenticate(
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
                // Check if it's Bearer token or exact match for X-Aura-Token
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

async fn handle_jsonrpc(
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

async fn handle_extension_add(
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

    match state.engine.add_task_with_sources(id, name, sources).await {
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
    let name = "rpc-download".to_string(); // In a real scenario, we'd infer this later
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
        .add_task_with_sources(id, name, sources)
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
