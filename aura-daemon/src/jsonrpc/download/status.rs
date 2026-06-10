use crate::jsonrpc::utils::{format_task_value, parse_gid, rpc_error, RpcResultExt};
use aura_core::orchestrator::Engine;
use serde_json::{json, Value};

pub async fn handle_tell_active(engine: &Engine) -> Result<Value, Value> {
    let active = engine.tell_active().await.rpc_map_err()?;

    let res: Vec<Value> = active
        .into_iter()
        .map(|t| {
            json!({
                "gid": t.id.0.to_string(),
                "status": format!("{:?}", t.phase).to_lowercase(),
                "totalLength": t.total_length.to_string(),
                "completedLength": t.completed_length.to_string(),
                "name": t.name,
                "selectedFiles": t.selected_files,
            })
        })
        .collect();

    Ok(json!(res))
}

pub async fn handle_tell_waiting(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let params = params.unwrap_or(json!([]));
    let offset = params[0].as_u64().unwrap_or(0) as usize;
    let num = params[1].as_u64().unwrap_or(10) as usize;
    let keys: Option<Vec<String>> = params
        .get(2)
        .and_then(|k| serde_json::from_value(k.clone()).ok());

    let active = engine.tell_active().await.rpc_map_err()?;

    let waiting: Vec<Value> = active
        .into_iter()
        .filter(|t| {
            t.phase == aura_core::task::DownloadPhase::Waiting
                || t.phase == aura_core::task::DownloadPhase::Paused
        })
        .skip(offset)
        .take(num)
        .map(|t| {
            format_task_value(crate::jsonrpc::utils::TaskValueParams {
                gid: &t.id.0.to_string(),
                status: &format!("{:?}", t.phase).to_lowercase(),
                name: &t.name,
                total_len: t.total_length,
                completed_len: t.completed_length,
                uploaded_len: t.uploaded_length(),
                uris: &t.subtasks.iter().map(|s| s.uri.clone()).collect::<Vec<_>>(),
                error_msg: None,
                keys: &keys,
                selected_files: t.selected_files.as_deref(),
            })
        })
        .collect();

    Ok(json!(waiting))
}

pub async fn handle_get_status(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let params = params.ok_or_else(|| rpc_error(-32602, "Invalid params"))?;
    let gid_str: String =
        serde_json::from_value(params[0].clone()).map_err(|_| rpc_error(-32602, "Invalid GID"))?;
    let keys: Option<Vec<String>> = params
        .get(1)
        .and_then(|k| serde_json::from_value(k.clone()).ok());

    let gid = gid_str.parse::<u64>().unwrap_or(0);

    let active = engine.tell_active().await.rpc_map_err()?;

    if let Some(t) = active.into_iter().find(|t| t.id.0 == gid) {
        return Ok(format_task_value(crate::jsonrpc::utils::TaskValueParams {
            gid: &t.id.0.to_string(),
            status: &format!("{:?}", t.phase).to_lowercase(),
            name: &t.name,
            total_len: t.total_length,
            completed_len: t.completed_length,
            uploaded_len: t.uploaded_length(),
            uris: &t
                .subtasks
                .iter()
                .map(|s| s.uri.clone())
                .collect::<Vec<String>>(),
            error_msg: None,
            keys: &keys,
            selected_files: t.selected_files.as_deref(),
        }));
    }

    let history = engine
        .tell_history(0, engine.config.load().limits.history_record_limit)
        .await
        .rpc_map_err()?;

    if let Some(rec) = history.into_iter().find(|r| r.id == gid_str) {
        return Ok(format_task_value(crate::jsonrpc::utils::TaskValueParams {
            gid: &rec.id,
            status: &rec.phase.to_lowercase(),
            name: &rec.name,
            total_len: rec.total_bytes,
            completed_len: rec.downloaded_bytes,
            uploaded_len: rec.uploaded_bytes,
            uris: &rec.uris,
            error_msg: rec.error.as_deref(),
            keys: &keys,
            selected_files: None,
        }));
    }

    Err(rpc_error(-32000, "Task not found"))
}

pub async fn handle_get_files(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let id = parse_gid(params.clone())?;

    let files = engine.get_files(id).await.rpc_map_err()?;

    match files {
        Some(f) => Ok(json!(f)),
        None => Err(rpc_error(
            -32000,
            "Files not available or not a BitTorrent task",
        )),
    }
}
