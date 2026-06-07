use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BandwidthSchedule {
    pub from: String,
    pub to: String,
    pub download_limit: u64,
    pub upload_limit: u64,
    #[serde(default)]
    pub days: Vec<String>,
    pub timezone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BandwidthConfig {
    pub global_download_limit: u64, // 0 for unlimited
    pub global_upload_limit: u64,
    pub per_task_download_limit: u64,
    pub per_task_upload_limit: u64,
    pub max_concurrent_downloads: usize,
    pub max_active_tasks: usize,
    pub min_connections_per_task: usize,
    pub max_connections_per_task: usize,
    pub adaptive_scaling_low_throughput: f64,
    pub adaptive_scaling_high_throughput: f64,
    pub refill_interval_ms: u64,
    pub schedule: Vec<BandwidthSchedule>,
}

impl Default for BandwidthConfig {
    fn default() -> Self {
        Self {
            global_download_limit: 0,
            global_upload_limit: 0,
            per_task_download_limit: 0,
            per_task_upload_limit: 0,
            max_concurrent_downloads: 10,
            max_active_tasks: 5,
            min_connections_per_task: 16,
            max_connections_per_task: 128,
            adaptive_scaling_low_throughput: 1024.0,
            adaptive_scaling_high_throughput: 1048576.0,
            refill_interval_ms: 100,
            schedule: Vec::new(),
        }
    }
}
