use aura_core::TaskId;
use serde_json::{json, Value};

pub fn rpc_error(code: i64, message: impl Into<String>) -> Value {
    json!({ "code": code, "message": message.into() })
}

pub trait RpcResultExt<T> {
    fn rpc_map_err(self) -> Result<T, Value>;
}

impl<T, E: std::fmt::Display> RpcResultExt<T> for Result<T, E> {
    fn rpc_map_err(self) -> Result<T, Value> {
        self.map_err(|e| rpc_error(-32000, e.to_string()))
    }
}

pub fn parse_gid(params: Option<Value>) -> Result<TaskId, Value> {
    let params = params.ok_or_else(|| rpc_error(-32602, "Invalid params"))?;
    let gid_str: String =
        serde_json::from_value(params[0].clone()).map_err(|_| rpc_error(-32602, "Invalid GID"))?;
    let gid = gid_str
        .parse::<u64>()
        .map_err(|_| rpc_error(-32602, "Invalid GID format"))?;
    Ok(TaskId(gid))
}

pub struct TaskValueParams<'a> {
    pub gid: &'a str,
    pub status: &'a str,
    pub name: &'a str,
    pub total_len: u64,
    pub completed_len: u64,
    pub uploaded_len: u64,
    pub uris: &'a [String],
    pub error_msg: Option<&'a str>,
    pub keys: &'a Option<Vec<String>>,
    pub selected_files: Option<&'a [bool]>,
}

pub fn format_task_value(params: TaskValueParams) -> Value {
    let mut map = serde_json::Map::new();

    let files_val = json!([{
        "index": "1",
        "path": params.name,
        "length": params.total_len.to_string(),
        "completedLength": params.completed_len.to_string(),
        "selected": "true",
        "uris": params.uris.iter().map(|u| json!({ "uri": u, "status": "used" })).collect::<Vec<Value>>(),
    }]);

    let err_code = if params.error_msg.is_some() { "1" } else { "0" };

    let all_fields = json!({
        "gid": params.gid.to_string(),
        "status": params.status.to_string(),
        "totalLength": params.total_len.to_string(),
        "completedLength": params.completed_len.to_string(),
        "uploadLength": params.uploaded_len.to_string(),
        "downloadSpeed": "0",
        "uploadSpeed": "0",
        "files": files_val,
        "errorCode": err_code.to_string(),
        "errorMessage": params.error_msg.unwrap_or("").to_string(),
        "name": params.name.to_string(),
        "selectedFiles": params.selected_files,
    });

    if let Some(ref k) = params.keys {
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
