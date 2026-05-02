//! task: Core representations of download tasks and their lifecycles.

use crate::TaskId;
use serde::{Deserialize, Serialize};

/// Represents the current lifecycle state of a Download Task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadPhase {
    MetadataExchange,
    Downloading,
    Paused,
    Complete,
    Error,
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
}

/// The high-level representation of a logical download operation.
/// A MetaTask can manage multiple SubTasks (sources).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetaTask {
    pub id: TaskId, // Unified ID for the logical file
    pub name: String,
    pub total_length: u64,
    pub completed_length: u64,
    pub phase: DownloadPhase,
    pub subtasks: Vec<SubTask>,
    pub pending_ranges: Vec<Range>,
    pub in_flight_ranges: Vec<(TaskId, Range)>, // (SubTaskID, Range)
}

use crate::bitfield::Bitfield;

/// Represents the serializable state of a MetaTask for persistence.
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskState {
    pub id: TaskId,
    pub name: String,
    pub phase: DownloadPhase,
    pub total_length: u64,
    pub completed_length: u64,
    pub subtasks: Vec<SubTask>,
    pub pending_ranges: Vec<Range>,
    pub bitfield: Option<Bitfield>,
}

impl MetaTask {
    pub fn to_state(&self, bitfield: Option<Bitfield>) -> TaskState {
        TaskState {
            id: self.id,
            name: self.name.clone(),
            phase: self.phase,
            total_length: self.total_length,
            completed_length: self.completed_length,
            subtasks: self.subtasks.clone(),
            pending_ranges: self.pending_ranges.clone(),
            bitfield,
        }
    }

    pub fn from_state(state: TaskState) -> Self {
        Self {
            id: state.id,
            name: state.name,
            phase: state.phase,
            total_length: state.total_length,
            completed_length: state.completed_length,
            subtasks: state.subtasks,
            pending_ranges: state.pending_ranges,
            in_flight_ranges: Vec::new(),
        }
    }

    pub fn new(id: TaskId, name: String, total_length: u64) -> Self {
        Self {
            id,
            name,
            total_length,
            completed_length: 0,
            phase: DownloadPhase::Downloading,
            subtasks: Vec::new(),
            pending_ranges: Vec::new(),
            in_flight_ranges: Vec::new(),
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
            phase: DownloadPhase::MetadataExchange,
        });
        sub_id
    }

    pub fn pick_range_for_subtask(&mut self, sub_id: TaskId) -> Option<Range> {
        if let Some(range) = self.pending_ranges.pop() {
            self.in_flight_ranges.push((sub_id, range));
            if let Some(sub) = self.subtasks.iter_mut().find(|s| s.id == sub_id) {
                sub.assigned_ranges.push(range);
            }
            return Some(range);
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
