use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TuiConfig {
    pub tick_rate_ms: u64,
    pub rpc_url: String,
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            tick_rate_ms: 500,
            rpc_url: "http://localhost:6800/jsonrpc".to_string(),
        }
    }
}
