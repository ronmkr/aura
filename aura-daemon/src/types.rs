use aura_core::orchestrator::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(rename = "jsonrpc")]
    pub _jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    pub id: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub already_exists: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ExtensionAddRequest {
    pub uri: String,
    pub mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

pub struct AppState {
    pub engine: Arc<Engine>,
    pub rpc_secret: Option<String>,
    pub metrics: Arc<super::metrics::DaemonMetrics>,
    pub rss_refresh_tx: Option<tokio::sync::mpsc::Sender<()>>,
}
