use crate::jsonrpc::utils::{parse_gid, rpc_error, RpcResultExt};
use aura_core::orchestrator::{Engine, TaskController};
use aura_core::TaskId;
use serde_json::{json, Value};

pub async fn handle_pause(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine.pause(gid).await.rpc_map_err()?;
    Ok(json!("OK"))
}

pub async fn handle_unpause(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine.resume(gid).await.rpc_map_err()?;
    Ok(json!("OK"))
}

pub async fn handle_force_recheck(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine.force_recheck(gid).await.rpc_map_err()?;
    Ok(json!("OK"))
}

pub async fn handle_refresh(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine.refresh(gid).await.rpc_map_err()?;
    Ok(json!("OK"))
}

pub async fn handle_remove(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine.remove(gid).await.rpc_map_err()?;
    Ok(json!("OK"))
}

pub async fn handle_change_option(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let params = params.ok_or_else(|| rpc_error(-32602, "Invalid params"))?;
    let gid_str: String =
        serde_json::from_value(params[0].clone()).map_err(|_| rpc_error(-32602, "Invalid GID"))?;
    let id = TaskId(
        gid_str
            .parse::<u64>()
            .map_err(|_| rpc_error(-32602, "Invalid GID format"))?,
    );

    let options = params
        .get(1)
        .ok_or_else(|| rpc_error(-32602, "Missing options"))?;

    let priority = options
        .get("priority")
        .and_then(|v| v.as_u64())
        .map(|p| p as u32);

    if let Some(p) = priority {
        if p > 5 {
            return Err(rpc_error(
                -32602,
                "Invalid priority: must be between 0 and 5",
            ));
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

    let seed_ratio = options.get("seed-ratio").and_then(|v| {
        if let Some(s) = v.as_str() {
            s.parse::<f32>().ok()
        } else {
            v.as_f64().map(|f| f as f32)
        }
    });

    let seed_time = options.get("seed-time").and_then(|v| {
        if let Some(s) = v.as_str() {
            s.parse::<u32>().ok()
        } else {
            v.as_u64().map(|n| n as u32)
        }
    });

    let streaming_mode = options
        .get("streaming-mode")
        .or_else(|| options.get("streamingMode"))
        .and_then(|v| v.as_bool());

    engine
        .change_option(
            id,
            priority,
            depends_on,
            seed_ratio,
            seed_time,
            streaming_mode,
        )
        .await
        .rpc_map_err()?;

    Ok(json!("OK"))
}

pub async fn handle_set_file_selection(
    engine: &Engine,
    params: Option<Value>,
) -> Result<Value, Value> {
    let id = parse_gid(params.clone())?;

    let params_val = params.ok_or_else(|| rpc_error(-32602, "Invalid params"))?;
    let selection: Vec<bool> = serde_json::from_value(params_val[1].clone())
        .map_err(|_| rpc_error(-32602, "Invalid boolean array"))?;

    engine
        .set_file_selection(id, selection)
        .await
        .rpc_map_err()?;

    Ok(json!("OK"))
}
