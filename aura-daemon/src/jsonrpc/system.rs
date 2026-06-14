use super::utils::format_task_value;
use crate::jsonrpc::utils::rpc_error;
use aura_core::orchestrator::{Engine, TaskQuerier};
use serde_json::{json, Value};

pub async fn handle_get_config(engine: &Engine) -> Result<Value, Value> {
    let config = engine
        .tell_config()
        .await
        .map_err(|e| rpc_error(-32000, e.to_string()))?;

    let now = chrono::Utc::now();
    let (_, _, active_schedule) =
        aura_core::config::BandwidthScheduler::effective_limits(&config.bandwidth, now);
    let next_transition =
        aura_core::config::BandwidthScheduler::next_transition(&config.bandwidth, now);

    let mut config_val =
        serde_json::to_value(&*config).map_err(|e| rpc_error(-32000, e.to_string()))?;

    if let Some(obj) = config_val.as_object_mut() {
        obj.insert(
            "active_schedule".to_string(),
            serde_json::to_value(&active_schedule).unwrap_or(Value::Null),
        );
        obj.insert(
            "next_transition".to_string(),
            next_transition.map(|t| t.to_rfc3339()).into(),
        );
    }

    Ok(config_val)
}

pub async fn handle_get_version() -> Result<Value, Value> {
    Ok(json!({
        "version": "1.36.0",
        "enabledFeatures": ["Digest", "GZip", "HTTPS", "MessageDigest", "Metalink"]
    }))
}

pub async fn handle_get_session_info() -> Result<Value, Value> {
    Ok(json!({
        "sessionId": "01234567-89ab-cdef-0123-456789abcdef"
    }))
}

pub async fn handle_tell_stopped(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let params = params.unwrap_or(json!([]));
    let offset = params[0].as_u64().unwrap_or(0) as usize;
    let num = params[1].as_u64().unwrap_or(10) as usize;
    let keys: Option<Vec<String>> = params
        .get(2)
        .and_then(|k| serde_json::from_value(k.clone()).ok());

    let records = engine
        .tell_history(offset, num)
        .await
        .map_err(|e| rpc_error(-32000, e.to_string()))?;

    let res: Vec<Value> = records
        .into_iter()
        .map(|rec| {
            format_task_value(crate::jsonrpc::utils::TaskValueParams {
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
                swarm_seeders: None,
                swarm_leechers: None,
                recheck_progress: 0.0,
            })
        })
        .collect();

    Ok(json!(res))
}

pub async fn handle_purge_download_result(engine: &Engine) -> Result<Value, Value> {
    let config = engine.config.load();
    aura_core::history::HistoryManager::purge_history(&config);
    Ok(json!("OK"))
}

pub async fn handle_remove_download_result(
    engine: &Engine,
    params: Option<Value>,
) -> Result<Value, Value> {
    let params = params.ok_or_else(|| rpc_error(-32602, "Invalid params"))?;
    let gid_str: String =
        serde_json::from_value(params[0].clone()).map_err(|_| rpc_error(-32602, "Invalid GID"))?;
    let config = engine.config.load();
    aura_core::history::HistoryManager::remove_record_by_gid(&config, &gid_str);
    Ok(json!("OK"))
}

pub async fn handle_save_session() -> Result<Value, Value> {
    Ok(json!("OK"))
}

pub async fn handle_shutdown(engine: &Engine) -> Result<Value, Value> {
    engine
        .shutdown()
        .await
        .map_err(|e| rpc_error(-32000, e.to_string()))?;
    Ok(json!("OK"))
}

pub async fn handle_change_global_option(
    engine: &Engine,
    params: Option<Value>,
) -> Result<Value, Value> {
    let params = params.ok_or_else(|| rpc_error(-32602, "Invalid params"))?;
    let options = params[0]
        .as_object()
        .ok_or_else(|| rpc_error(-32602, "Invalid options"))?;

    let mut current_config = (*engine
        .tell_config()
        .await
        .map_err(|e| rpc_error(-32000, e.to_string()))?)
    .clone();

    if let Some(dl_limit_str) = options.get("max-overall-download-limit") {
        if let Some(limit_str) = dl_limit_str.as_str() {
            if let Ok(limit) = limit_str.parse::<u64>() {
                current_config.bandwidth.global_download_limit = limit;
            }
        }
    }

    if let Some(ul_limit_str) = options.get("max-overall-upload-limit") {
        if let Some(limit_str) = ul_limit_str.as_str() {
            if let Ok(limit) = limit_str.parse::<u64>() {
                current_config.bandwidth.global_upload_limit = limit;
            }
        }
    }

    engine
        .reload_config(current_config)
        .await
        .map_err(|e| rpc_error(-32000, e.to_string()))?;

    Ok(json!("OK"))
}

pub async fn handle_get_global_stat(engine: &Engine) -> Result<Value, Value> {
    let active = engine.tell_active().await.unwrap_or_default();
    let num_active = active
        .iter()
        .filter(|t| t.phase == aura_core::task::DownloadPhase::Downloading)
        .count();
    let num_waiting = active
        .iter()
        .filter(|t| t.phase == aura_core::task::DownloadPhase::Waiting)
        .count();

    let history = engine
        .tell_history(0, engine.config.load().limits.history_record_limit)
        .await
        .unwrap_or_default();
    let num_stopped = history.len();

    let config = engine.config.load();
    let watch_folder_active = config.storage.watch_dir.is_some() && {
        if let Some(ref path_str) = config.storage.watch_dir {
            std::path::Path::new(path_str).exists()
        } else {
            false
        }
    };

    let last_ingested = {
        let guard = engine.last_ingested_file.lock().await;
        guard.clone().unwrap_or_else(|| "".to_string())
    };

    Ok(json!({
        "downloadSpeed": "0",
        "uploadSpeed": "0",
        "numActive": num_active.to_string(),
        "numWaiting": num_waiting.to_string(),
        "numStopped": num_stopped.to_string(),
        "watchFolderActive": watch_folder_active,
        "lastIngestedFile": last_ingested,
    }))
}
