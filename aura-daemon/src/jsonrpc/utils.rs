use aura_core::TaskId;
use serde_json::{json, Value};

pub fn parse_gid(params: Option<Value>) -> Result<TaskId, Value> {
    let params = params.ok_or_else(|| json!({ "code": -32602, "message": "Invalid params" }))?;
    let gid_str: String = serde_json::from_value(params[0].clone())
        .map_err(|_| json!({ "code": -32602, "message": "Invalid GID" }))?;
    let gid = gid_str
        .parse::<u64>()
        .map_err(|_| json!({ "code": -32602, "message": "Invalid GID format" }))?;
    Ok(TaskId(gid))
}

#[allow(clippy::too_many_arguments)]
pub fn format_task_value(
    gid: &str,
    status: &str,
    name: &str,
    total_len: u64,
    completed_len: u64,
    uploaded_len: u64,
    uris: &[String],
    error_msg: Option<&str>,
    keys: &Option<Vec<String>>,
) -> Value {
    let mut map = serde_json::Map::new();

    let files_val = json!([{
        "index": "1",
        "path": name,
        "length": total_len.to_string(),
        "completedLength": completed_len.to_string(),
        "selected": "true",
        "uris": uris.iter().map(|u| json!({ "uri": u, "status": "used" })).collect::<Vec<Value>>(),
    }]);

    let err_code = if error_msg.is_some() { "1" } else { "0" };

    let all_fields = json!({
        "gid": gid.to_string(),
        "status": status.to_string(),
        "totalLength": total_len.to_string(),
        "completedLength": completed_len.to_string(),
        "uploadLength": uploaded_len.to_string(),
        "downloadSpeed": "0",
        "uploadSpeed": "0",
        "files": files_val,
        "errorCode": err_code.to_string(),
        "errorMessage": error_msg.unwrap_or("").to_string(),
        "name": name.to_string(),
    });

    if let Some(ref k) = keys {
        for key in k {
            if let Some(val) = all_fields.get(key) {
                map.insert(key.clone(), val.clone());
            } else if key == "dir" {
                map.insert("dir".to_string(), json!(""));
            } else {
                map.insert(key.clone(), json!(""));
            }
        }
        Value::Object(map)
    } else {
        all_fields
    }
}
