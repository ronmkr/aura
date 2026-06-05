use super::utils::{format_task_value, parse_gid};
use aura_core::net_util::validate_download_uri;
use aura_core::orchestrator::Engine;
use aura_core::task::TaskType;
use aura_core::TaskId;
use serde_json::{json, Value};

pub async fn handle_add_uri(
    engine: &Engine,
    params: Option<Value>,
) -> Result<(Value, Option<bool>), Value> {
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
        Err(e) => Err(json!({ "code": -32000, "message": e.to_string() })),
    }
}

pub async fn handle_tell_active(engine: &Engine) -> Result<Value, Value> {
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

pub async fn handle_pause(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine
        .pause(gid)
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;
    Ok(json!("OK"))
}

pub async fn handle_unpause(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine
        .resume(gid)
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;
    Ok(json!("OK"))
}

pub async fn handle_remove(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let gid = parse_gid(params)?;
    engine
        .remove(gid)
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;
    Ok(json!("OK"))
}

pub async fn handle_change_option(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
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

    engine
        .change_option(id, priority, depends_on, seed_ratio, seed_time)
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;

    Ok(json!("OK"))
}

pub async fn handle_tell_waiting(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let params = params.unwrap_or(json!([]));
    let offset = params[0].as_u64().unwrap_or(0) as usize;
    let num = params[1].as_u64().unwrap_or(10) as usize;
    let keys: Option<Vec<String>> = params
        .get(2)
        .and_then(|k| serde_json::from_value(k.clone()).ok());

    let active = engine
        .tell_active()
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;

    let waiting: Vec<Value> = active
        .into_iter()
        .filter(|t| {
            t.phase == aura_core::task::DownloadPhase::Waiting
                || t.phase == aura_core::task::DownloadPhase::Paused
        })
        .skip(offset)
        .take(num)
        .map(|t| {
            format_task_value(
                &t.id.0.to_string(),
                &format!("{:?}", t.phase).to_lowercase(),
                &t.name,
                t.total_length,
                t.completed_length,
                t.uploaded_length,
                &t.subtasks
                    .iter()
                    .map(|s| s.uri.clone())
                    .collect::<Vec<String>>(),
                None,
                &keys,
            )
        })
        .collect();

    Ok(json!(waiting))
}

pub async fn handle_get_status(engine: &Engine, params: Option<Value>) -> Result<Value, Value> {
    let params = params.ok_or_else(|| json!({ "code": -32602, "message": "Invalid params" }))?;
    let gid_str: String = serde_json::from_value(params[0].clone())
        .map_err(|_| json!({ "code": -32602, "message": "Invalid GID" }))?;
    let keys: Option<Vec<String>> = params
        .get(1)
        .and_then(|k| serde_json::from_value(k.clone()).ok());

    let gid = gid_str.parse::<u64>().unwrap_or(0);

    let active = engine
        .tell_active()
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;

    if let Some(t) = active.into_iter().find(|t| t.id.0 == gid) {
        return Ok(format_task_value(
            &t.id.0.to_string(),
            &format!("{:?}", t.phase).to_lowercase(),
            &t.name,
            t.total_length,
            t.completed_length,
            t.uploaded_length,
            &t.subtasks
                .iter()
                .map(|s| s.uri.clone())
                .collect::<Vec<String>>(),
            None,
            &keys,
        ));
    }

    let history = engine
        .tell_history(0, 100000)
        .await
        .map_err(|e| json!({ "code": -32000, "message": e.to_string() }))?;

    if let Some(rec) = history.into_iter().find(|r| r.id == gid_str) {
        return Ok(format_task_value(
            &rec.id,
            &rec.phase.to_lowercase(),
            &rec.name,
            rec.total_bytes,
            rec.downloaded_bytes,
            rec.uploaded_bytes,
            &rec.uris,
            rec.error.as_deref(),
            &keys,
        ));
    }

    Err(json!({ "code": -32000, "message": "Task not found" }))
}
