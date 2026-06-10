use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub download_dir: String,
    pub sandbox_root: Option<String>,
    pub cache_size_mb: u32,
    pub preallocate: bool,
    pub allocation_mode: String, // "none", "prealloc", "falloc"
    pub save_session_interval_secs: u64,
    pub flush_interval_secs: u64,
    pub io_deadline_ms: u64,
    pub read_ahead_kb: u32,
    pub write_buffer_kb: u32,
    pub memory_limit_mb: u32,
    pub memory_safety_margin_mb: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            download_dir: ".".to_string(),
            sandbox_root: None,
            cache_size_mb: 16,
            preallocate: true,
            allocation_mode: "falloc".to_string(),
            save_session_interval_secs: 10,
            flush_interval_secs: 3,
            io_deadline_ms: 500,
            read_ahead_kb: 128,
            write_buffer_kb: 256,
            memory_limit_mb: 512,
            memory_safety_margin_mb: 51,
        }
    }
}
