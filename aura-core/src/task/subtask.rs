use super::phase::{DownloadPhase, TaskType};
use super::range::Range;
use crate::TaskId;
use serde::{Deserialize, Serialize};

/// A sub-segment of a download, managed by a specific protocol worker.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubTask {
    pub id: TaskId, // Unique for each subtask
    pub task_type: TaskType,
    pub uri: String,
    pub assigned_ranges: Vec<Range>,
    pub total_length: u64,
    pub completed_length: u64,
    pub active: bool,
    pub phase: DownloadPhase,
    pub target_concurrency: usize,
    pub recent_bytes_downloaded: u64,
    pub ewma_throughput: f64,
    pub retry_count: u32,
}
