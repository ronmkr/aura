//! task: Core representations of download tasks and their lifecycles.

use crate::TaskId;
use serde::{Deserialize, Serialize};

/// Represents the current lifecycle state of a Download Task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadPhase {
    MetadataExchange,
    Downloading,
    Verifying,
    Paused,
    Complete,
    Error,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskType {
    Http,
    BitTorrent,
    Ftp,
}

/// Represents a byte range [start, end)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Range {
    pub start: u64,
    pub end: u64,
}

impl Range {
    pub fn length(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}

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

/// The high-level representation of a logical download operation.
/// A MetaTask can manage multiple SubTasks (sources).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaTask {
    pub id: TaskId, // Unified ID for the logical file
    pub name: String,
    pub total_length: u64,
    pub completed_length: u64,
    pub uploaded_length: u64,
    pub phase: DownloadPhase,
    pub priority: u32, // 0 = highest, 100 = default
    pub streaming_mode: bool,
    pub subtasks: Vec<SubTask>,
    pub pending_ranges: Vec<Range>,
    pub in_flight_ranges: Vec<(TaskId, Range)>, // (SubTaskID, Range)
    pub checksum: Option<crate::Checksum>,
    pub seeding_start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub blacklisted_uris: Vec<String>,
}

use crate::bitfield::Bitfield;

/// Represents the serializable state of a MetaTask for persistence.
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskState {
    pub id: TaskId,
    pub name: String,
    pub phase: DownloadPhase,
    pub priority: u32,
    pub streaming_mode: bool,
    pub total_length: u64,
    pub completed_length: u64,
    pub uploaded_length: u64,
    pub subtasks: Vec<SubTask>,
    pub pending_ranges: Vec<Range>,
    pub bitfield: Option<Bitfield>,
    pub checksum: Option<crate::Checksum>,
    pub seeding_start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub blacklisted_uris: Option<Vec<String>>,
}

impl MetaTask {
    pub fn to_state(&self, bitfield: Option<Bitfield>) -> TaskState {
        TaskState {
            id: self.id,
            name: self.name.clone(),
            phase: self.phase,
            priority: self.priority,
            streaming_mode: self.streaming_mode,
            total_length: self.total_length,
            completed_length: self.completed_length,
            uploaded_length: self.uploaded_length,
            subtasks: self.subtasks.clone(),
            pending_ranges: self.pending_ranges.clone(),
            bitfield,
            checksum: self.checksum.clone(),
            seeding_start_time: self.seeding_start_time,
            blacklisted_uris: Some(self.blacklisted_uris.clone()),
        }
    }

    pub fn from_state(state: TaskState) -> Self {
        Self {
            id: state.id,
            name: state.name,
            phase: state.phase,
            priority: state.priority,
            streaming_mode: state.streaming_mode,
            total_length: state.total_length,
            completed_length: state.completed_length,
            uploaded_length: state.uploaded_length,
            subtasks: state.subtasks,
            pending_ranges: state.pending_ranges,
            in_flight_ranges: Vec::new(),
            checksum: state.checksum,
            seeding_start_time: state.seeding_start_time,
            blacklisted_uris: state.blacklisted_uris.unwrap_or_default(),
        }
    }

    pub fn new(id: TaskId, name: String, total_length: u64) -> Self {
        Self {
            id,
            name,
            total_length,
            completed_length: 0,
            uploaded_length: 0,
            phase: DownloadPhase::Downloading,
            priority: 100,
            streaming_mode: false,
            subtasks: Vec::new(),
            pending_ranges: Vec::new(),
            in_flight_ranges: Vec::new(),
            checksum: None,
            seeding_start_time: None,
            blacklisted_uris: Vec::new(),
        }
    }

    pub fn generate_ranges(&mut self, num_ranges: usize) {
        if self.total_length == 0 {
            return;
        }
        self.pending_ranges.clear();

        // Generate granular ranges
        let actual_num_ranges = std::cmp::max(num_ranges, 32);
        let granular_size = self.total_length.div_ceil(actual_num_ranges as u64);

        for i in 0..actual_num_ranges {
            let start = i as u64 * granular_size;
            let end = std::cmp::min(start + granular_size, self.total_length);
            if start < end {
                self.pending_ranges.push(Range { start, end });
            }
        }
        // Reverse so we can pop from the end (efficient)
        self.pending_ranges.reverse();
    }

    pub fn add_subtask(&mut self, uri: String, task_type: TaskType) -> TaskId {
        let sub_id = TaskId(rand::random());
        self.subtasks.push(SubTask {
            id: sub_id,
            task_type,
            uri,
            assigned_ranges: Vec::new(),
            total_length: 0,
            completed_length: 0,
            active: true,
            phase: DownloadPhase::Downloading,
            target_concurrency: 1,
            recent_bytes_downloaded: 0,
            ewma_throughput: 0.0,
            retry_count: 0,
        });

        sub_id
    }

    pub fn pick_range_for_subtask(&mut self, sub_id: TaskId) -> Option<Range> {
        // 1. Try to pick from pending ranges first
        if let Some(range) = self.pending_ranges.pop() {
            self.in_flight_ranges.push((sub_id, range));
            if let Some(sub) = self.subtasks.iter_mut().find(|s| s.id == sub_id) {
                sub.assigned_ranges.push(range);
            }
            return Some(range);
        }

        // 2. Work Stealing / Racing (ADR 0005)
        // If no pending ranges, look for "lagging" in-flight ranges to race against.
        // A range is lagging if its assigned subtask's throughput is significantly below average.
        let avg_throughput = {
            let active_subs: Vec<_> = self
                .subtasks
                .iter()
                .filter(|s| s.ewma_throughput > 0.0)
                .collect();
            if active_subs.is_empty() {
                0.0
            } else {
                active_subs.iter().map(|s| s.ewma_throughput).sum::<f64>()
                    / active_subs.len() as f64
            }
        };

        if avg_throughput > 0.0 {
            let mut candidates = Vec::new();
            for (assigned_sub_id, range) in &self.in_flight_ranges {
                // Don't race against yourself
                if *assigned_sub_id == sub_id {
                    continue;
                }

                if let Some(other_sub) = self.subtasks.iter().find(|s| s.id == *assigned_sub_id) {
                    // Race if the other subtask is 3x slower than average
                    if other_sub.ewma_throughput < (avg_throughput / 3.0) {
                        candidates.push((*assigned_sub_id, *range));
                    }
                }
            }

            if let Some((_other_id, range)) = candidates.first() {
                let range = *range;
                self.in_flight_ranges.push((sub_id, range));
                if let Some(sub) = self.subtasks.iter_mut().find(|s| s.id == sub_id) {
                    sub.assigned_ranges.push(range);
                }
                tracing::info!(%sub_id, ?range, "Racing/Stealing range from slow source");
                return Some(range);
            }
        }

        None
    }

    pub fn mark_range_complete(&mut self, sub_id: TaskId, range: Range) {
        self.in_flight_ranges
            .retain(|(sid, r)| *sid != sub_id || *r != range);
        if let Some(sub) = self.subtasks.iter_mut().find(|s| s.id == sub_id) {
            sub.assigned_ranges.retain(|r| *r != range);
            sub.completed_length += range.length();
        }
    }

    pub fn is_complete(&self) -> bool {
        self.completed_length >= self.total_length && self.total_length > 0
    }

    pub fn progress(&self) -> f64 {
        if self.total_length == 0 {
            0.0
        } else {
            (self.completed_length as f64 / self.total_length as f64) * 100.0
        }
    }
}
