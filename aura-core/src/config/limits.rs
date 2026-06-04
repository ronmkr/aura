use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    pub allow_duplicate_uris: bool,
    pub max_active_tasks: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            allow_duplicate_uris: false,
            max_active_tasks: 500,
        }
    }
}
