use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BulkConfig {
    pub max_scan_depth: usize,
}

impl Default for BulkConfig {
    fn default() -> Self {
        Self { max_scan_depth: 10 }
    }
}
