use crate::jsonrpc::utils::rpc_error;
use aura_core::net_util::validate_download_uri;
use aura_core::orchestrator::Engine;
use aura_core::task::TaskType;
use aura_core::TaskId;
use serde_json::{json, Value};

pub async fn handle_add_uri(
    engine: &Engine,
    params: Option<Value>,
) -> Result<(Value, Option<bool>), Value> {
    let params = params.ok_or_else(|| rpc_error(-32602, "Invalid params"))?;
    let uris: Vec<String> =
        serde_json::from_value(params[0].clone()).map_err(|_| rpc_error(-32602, "Invalid URIs"))?;

    if uris.is_empty() {
        return Err(rpc_error(-32602, "Empty URI list"));
    }

    for uri in &uris {
        if let Err(e) = validate_download_uri(uri) {
            return Err(rpc_error(-32602, e.to_string()));
        }
    }

    let mut priority = 3;
    let mut streaming_mode = false;
    let mut depends_on = Vec::new();

    if let Some(options) = params.get(1) {
        if let Some(p) = options.get("priority").and_then(|v| v.as_u64()) {
            if p > 5 {
                return Err(rpc_error(
                    -32602,
                    "Invalid priority: must be between 0 and 5",
                ));
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

    let id = TaskId::random();
    let name = crate::jsonrpc::download::DEFAULT_RPC_NAME.to_string();
    let mut sources = Vec::new();
    for uri in uris {
        let ttype = if let Some(detected) =
            aura_core::orchestrator::protocol_detector::ProtocolDetector::detect(&uri).await
        {
            detected.to_task_type()
        } else {
            TaskType::Http
        };
        sources.push((uri, ttype));
    }

    let add_result = engine
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
        .await;

    match add_result {
        Ok(_) => Ok((json!(id.0.to_string()), None)),
        Err(aura_core::Error::DuplicateTask(existing_id)) => {
            Ok((json!(existing_id.0.to_string()), Some(true)))
        }
        Err(e) => Err(rpc_error(-32000, e.to_string())),
    }
}

pub async fn handle_add_from_folder(
    engine: &Engine,
    params: Option<Value>,
) -> Result<Value, Value> {
    let params = params.ok_or_else(|| rpc_error(-32602, "Invalid params"))?;
    let dir = params[0]
        .as_str()
        .ok_or_else(|| rpc_error(-32602, "Invalid directory path"))?;
    let recursive = params[1].as_bool().unwrap_or(false);

    let ids = engine
        .add_from_folder(None, dir, recursive)
        .await
        .map_err(|e| rpc_error(-32000, e.to_string()))?;

    let res: Vec<String> = ids.into_iter().map(|id| id.0.to_string()).collect();
    Ok(json!(res))
}

pub async fn handle_add_from_file(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let params = params.ok_or_else(|| rpc_error(-32602, "Invalid params"))?;
    let path = params[0]
        .as_str()
        .ok_or_else(|| rpc_error(-32602, "Invalid file path"))?;

    let ids = engine
        .add_from_file(None, path)
        .await
        .map_err(|e| rpc_error(-32000, e.to_string()))?;

    let res: Vec<String> = ids.into_iter().map(|id| id.0.to_string()).collect();
    Ok(json!(res))
}
