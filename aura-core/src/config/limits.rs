use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LimitsConfig {
    pub allow_duplicate_uris: bool,
    pub max_active_tasks: usize,
    pub event_channel_capacity: usize,
    pub command_channel_capacity: usize,
    pub storage_channel_capacity: usize,
    pub history_record_limit: usize,
    pub history_rotation_mb: f64,
    pub history_rotation_records: usize,
    pub history_retention_records: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            allow_duplicate_uris: false,
            max_active_tasks: 500,
            event_channel_capacity: 1024,
            command_channel_capacity: 100,
            storage_channel_capacity: 100,
            history_record_limit: 100000,
            history_rotation_mb: 10.0,
            history_rotation_records: 10000,
            history_retention_records: 5000,
        }
    }
}
