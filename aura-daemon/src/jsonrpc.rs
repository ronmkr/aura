use super::types::{AppState, JsonRpcRequest, JsonRpcResponse};
use aura_core::net_util::validate_download_uri;
use aura_core::orchestrator::Engine;
use aura_core::task::TaskType;
use aura_core::TaskId;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::info;

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
        "aria2.changeOption" => handle_change_option(&state.engine, payload.params).await,
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

async fn handle_add_uri(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let params = params.ok_or_else(|| json!({ "code": -32602, "message": "Invalid params" }))?;
    let uris: Vec<String> = serde_json::from_value(params[0].clone())
        .map_err(|_| json!({ "code": -32602, "message": "Invalid URIs" }))?;

    if uris.is_empty() {
        return Err(json!({ "code": -32602, "message": "Empty URI list" }));
    }

    // Validate each URI before entering the pipeline (ADR-0059: SSRF mitigation)
    // Blocks file://, data:, javascript:, RFC1918, loopback, and link-local addresses.
    for uri in &uris {
        if let Err(e) = validate_download_uri(uri) {
            return Err(json!({ "code": -32602, "message": e.to_string() }));
        }
    }

    let mut priority = 3;
    let mut streaming_mode = false;
    let mut depends_on = Vec::new();

    if let Some(options) = params.get(1) {
        if let Some(p) = options.get("priority").and_then(|v| v.as_u64()) {
            if p > 5 {
                return Err(
                    json!({ "code": -32602, "message": "Invalid priority: must be between 0 and 5" }),
                );
            }
            priority = p as u32;
        }
        if let Some(s) = options.get("streamingMode").and_then(|v| v.as_bool()) {
            streaming_mode = s;
        }
        if let Some(deps) = options
            .get("depends_on")
            .or_else(|| options.get("dependsOn"))
            .and_then(|v| v.as_array())
        {
            for dep in deps {
                if let Some(dep_str) = dep.as_str() {
                    if let Ok(dep_id) = dep_str.parse::<u64>() {
                        depends_on.push(TaskId(dep_id));
                    }
                } else if let Some(dep_num) = dep.as_u64() {
                    depends_on.push(TaskId(dep_num));
                }
            }
        }
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
        .add_task_with_options(aura_core::orchestrator::command::AddTaskArgs {
            id,
            tenant_id: None,
            name,
            sources,
            checksum: None,
            priority,
            streaming_mode,
            depends_on,
            follow_on: None,
        })
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

async fn handle_change_option(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let params = params.ok_or_else(|| json!({ "code": -32602, "message": "Invalid params" }))?;
    let gid_str: String = serde_json::from_value(params[0].clone())
        .map_err(|_| json!({ "code": -32602, "message": "Invalid GID" }))?;
    let id = TaskId(
        gid_str
            .parse::<u64>()
            .map_err(|_| json!({ "code": -32602, "message": "Invalid GID format" }))?,
    );

    let options = params
        .get(1)
        .ok_or_else(|| json!({ "code": -32602, "message": "Missing options" }))?;

    let priority = options
        .get("priority")
        .and_then(|v| v.as_u64())
        .map(|p| p as u32);

    if let Some(p) = priority {
        if p > 5 {
            return Err(
                json!({ "code": -32602, "message": "Invalid priority: must be between 0 and 5" }),
            );
        }
    }

    let mut depends_on = None;
    if let Some(deps) = options
        .get("depends_on")
        .or_else(|| options.get("dependsOn"))
        .and_then(|v| v.as_array())
    {
        let mut list = Vec::new();
        for dep in deps {
            if let Some(dep_str) = dep.as_str() {
                if let Ok(dep_id) = dep_str.parse::<u64>() {
                    list.push(TaskId(dep_id));
                }
            } else if let Some(dep_num) = dep.as_u64() {
                list.push(TaskId(dep_num));
            }
        }
        depends_on = Some(list);
    }

    engine
        .change_option(id, priority, depends_on)
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;

    Ok(json!("OK"))
}
