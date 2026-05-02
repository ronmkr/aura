use aura_core::orchestrator::Engine;
use aura_core::task::TaskType;
use aura_core::TaskId;
use axum::{extract::State, routing::post, Json, Router};
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

struct AppState {
    engine: Arc<Engine>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Starting Aura Daemon");

    // Bootstrap the engine
    let config = aura_core::Config::from_file("Aura.toml").unwrap_or_default();
    let (engine, orchestrator, storage) = Engine::new(config).await.unwrap();
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

    let state = Arc::new(AppState { engine });

    let app = Router::new()
        .route("/jsonrpc", post(handle_jsonrpc))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6800").await?;
    info!("RPC Server listening on http://0.0.0.0:6800/jsonrpc");
    axum::serve(listener, app).await?;

    Ok(())
}

async fn handle_jsonrpc(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<JsonRpcRequest>,
) -> Json<JsonRpcResponse> {
    info!("RPC Method: {}", payload.method);

    let result = match payload.method.as_str() {
        "aria2.addUri" => handle_add_uri(&state.engine, payload.params).await,
        "aria2.tellActive" => handle_tell_active(&state.engine).await,
        "aria2.pause" => handle_pause(&state.engine, payload.params).await,
        "aria2.unpause" => handle_unpause(&state.engine, payload.params).await,
        "aria2.remove" => handle_remove(&state.engine, payload.params).await,
        _ => Err(json!({ "code": -32601, "message": "Method not found" })),
    };

    match result {
        Ok(res) => Json(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: Some(res),
            error: None,
            id: payload.id,
        }),
        Err(err) => Json(JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(err),
            id: payload.id,
        }),
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
        .unpause(gid)
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

fn parse_gid(params: Option<Value>) -> Result<TaskId, Value> {
    let params = params.ok_or_else(|| json!({ "code": -32602, "message": "Invalid params" }))?;
    let gid_str: String = serde_json::from_value(params[0].clone())
        .map_err(|_| json!({ "code": -32602, "message": "Invalid GID" }))?;
    let gid = gid_str
        .parse::<u64>()
        .map_err(|_| json!({ "code": -32602, "message": "Invalid GID format" }))?;
    Ok(TaskId(gid))
}
