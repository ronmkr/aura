use super::extension::TaskExtension;
use super::phase::{DownloadPhase, FollowOnAction, TaskType};
use super::range::Range;
use super::subtask::SubTask;
use crate::bitfield::Bitfield;
use crate::{TaskId, TenantId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// The high-level representation of a logical download operation.
/// A MetaTask can manage multiple SubTasks (sources).
#[derive(Debug, Clone)]
pub struct MetaTask {
    pub id: TaskId, // Unified ID for the logical file
    pub tenant_id: Option<TenantId>,
    pub name: String,
    pub total_length: u64,
    pub completed_length: u64,
    pub phase: DownloadPhase,
    pub priority: u32, // 0 = highest, 5 = lowest, default = 3
    pub streaming_mode: bool,
    pub range_supported: bool,
    pub follow_on: Option<FollowOnAction>,
    pub subtasks: Vec<SubTask>,
    pub pending_ranges: Vec<Range>,
    pub in_flight_ranges: Vec<(TaskId, Range)>, // (SubTaskID, Range)
    pub checksum: Option<crate::Checksum>,
    pub blacklisted_uris: Vec<String>,
    pub extensions: HashMap<String, Arc<dyn TaskExtension>>,
    pub depends_on: Vec<TaskId>,
    pub stall_ticks: u32,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub selected_files: Option<Vec<bool>>,
}

/// Represents the serializable state of a MetaTask for persistence.
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskState {
    pub id: TaskId,
    pub tenant_id: Option<TenantId>,
    pub name: String,
    pub phase: DownloadPhase,
    pub priority: u32,
    pub streaming_mode: bool,
    pub range_supported: bool,
    pub follow_on: Option<FollowOnAction>,
    pub total_length: u64,
    pub completed_length: u64,
    pub subtasks: Vec<SubTask>,
    pub pending_ranges: Vec<Range>,
    pub bitfield: Option<Bitfield>,
    pub checksum: Option<crate::Checksum>,
    pub blacklisted_uris: Option<Vec<String>>,
    pub depends_on: Option<Vec<TaskId>>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default)]
    pub etag: Option<String>,
    #[serde(default)]
    pub last_modified: Option<String>,
    #[serde(default)]
    pub selected_files: Option<Vec<bool>>,
}

impl MetaTask {
    pub fn to_state(&self, bitfield: Option<Bitfield>) -> TaskState {
        TaskState {
            id: self.id,
            tenant_id: self.tenant_id.clone(),
            name: self.name.clone(),
            phase: self.phase,
            priority: self.priority,
            streaming_mode: self.streaming_mode,
            range_supported: self.range_supported,
            follow_on: self.follow_on.clone(),
            total_length: self.total_length,
            completed_length: self.completed_length,
            subtasks: self.subtasks.clone(),
            pending_ranges: self.pending_ranges.clone(),
            bitfield,
            checksum: self.checksum.clone(),
            blacklisted_uris: Some(self.blacklisted_uris.clone()),
            depends_on: Some(self.depends_on.clone()),
            created_at: self.created_at,
            etag: self.etag.clone(),
            last_modified: self.last_modified.clone(),
            selected_files: self.selected_files.clone(),
        }
    }

    pub fn from_state(state: TaskState) -> Self {
        Self {
            id: state.id,
            tenant_id: state.tenant_id,
            name: state.name,
            phase: state.phase,
            priority: state.priority,
            streaming_mode: state.streaming_mode,
            range_supported: state.range_supported,
            follow_on: state.follow_on,
            total_length: state.total_length,
            completed_length: state.completed_length,
            subtasks: state.subtasks,
            pending_ranges: state.pending_ranges,
            in_flight_ranges: Vec::new(),
            checksum: state.checksum,
            blacklisted_uris: state.blacklisted_uris.unwrap_or_default(),
            extensions: HashMap::new(),
            depends_on: state.depends_on.unwrap_or_default(),
            stall_ticks: 0,
            created_at: state.created_at,
            etag: state.etag,
            last_modified: state.last_modified,
            selected_files: state.selected_files,
        }
    }

    pub fn new(id: TaskId, name: String, total_length: u64) -> Self {
        Self {
            id,
            tenant_id: None,
            name,
            total_length,
            completed_length: 0,
            phase: DownloadPhase::Downloading,
            priority: crate::config::LimitsConfig::default().default_task_priority,
            streaming_mode: false,
            range_supported: true, // Assume supported until proven otherwise
            follow_on: None,
            subtasks: Vec::new(),
            pending_ranges: Vec::new(),
            in_flight_ranges: Vec::new(),
            checksum: None,
            blacklisted_uris: Vec::new(),
            extensions: HashMap::new(),
            depends_on: Vec::new(),
            stall_ticks: 0,
            created_at: Some(chrono::Utc::now()),
            etag: None,
            last_modified: None,
            selected_files: None,
        }
    }

    pub fn generate_ranges(&mut self, num_ranges: usize, bitfield: Option<&Bitfield>) {
        if self.total_length == 0 {
            return;
        }
        self.pending_ranges.clear();

        if let Some(bf) = bitfield {
            let num_pieces = bf.len();
            let piece_len = self.total_length.div_ceil(num_pieces as u64);

            for i in 0..num_pieces {
                if !bf.get(i) {
                    let start = i as u64 * piece_len;
                    let end = std::cmp::min(start + piece_len, self.total_length);
                    if start < end {
                        self.pending_ranges.push(Range { start, end });
                    }
                }
            }
        } else {
            // Generate granular ranges for new tasks
            let actual_num_ranges = std::cmp::max(num_ranges, 32);
            let granular_size = self.total_length.div_ceil(actual_num_ranges as u64);

            for i in 0..actual_num_ranges {
                let start = i as u64 * granular_size;
                let end = std::cmp::min(start + granular_size, self.total_length);
                if start < end {
                    self.pending_ranges.push(Range { start, end });
                }
            }
        }
        // Reverse so we can pop from the end (efficient)
        self.pending_ranges.reverse();
    }

    pub fn add_subtask(&mut self, uri: String, task_type: TaskType) -> TaskId {
        let sub_id = TaskId::random();
        self.subtasks.push(SubTask {
            id: sub_id,
            task_type,
            uri,
            assigned_ranges: Vec::new(),
            total_length: 0,
            completed_length: 0,
            active: true,
            phase: DownloadPhase::Downloading,
            target_concurrency: 8,
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
        if !self.range_supported {
            return None;
        }

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
        if self.total_length > 0 {
            self.completed_length >= self.total_length
        } else {
            self.pending_ranges.is_empty()
                && self.in_flight_ranges.is_empty()
                && self.completed_length > 0
        }
    }

    pub fn progress(&self) -> f64 {
        if self.total_length == 0 {
            if self.is_complete() {
                100.0
            } else {
                0.0
            }
        } else {
            (self.completed_length as f64 / self.total_length as f64) * 100.0
        }
    }

    pub fn uploaded_length(&self) -> u64 {
        if let Some(ext) = self
            .extensions
            .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
        {
            if let Some(bt) = ext
                .as_any()
                .downcast_ref::<crate::worker::bittorrent::task::BtTask>()
            {
                return bt
                    .state
                    .uploaded_length
                    .load(std::sync::atomic::Ordering::Relaxed);
            }
        }
        0
    }

    pub fn seed_ratio(&self) -> Option<f32> {
        if let Some(ext) = self
            .extensions
            .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
        {
            if let Some(bt) = ext
                .as_any()
                .downcast_ref::<crate::worker::bittorrent::task::BtTask>()
            {
                return *bt.state.seed_ratio.lock().unwrap();
            }
        }
        None
    }

    pub fn seed_time(&self) -> Option<u32> {
        if let Some(ext) = self
            .extensions
            .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
        {
            if let Some(bt) = ext
                .as_any()
                .downcast_ref::<crate::worker::bittorrent::task::BtTask>()
            {
                return *bt.state.seed_time.lock().unwrap();
            }
        }
        None
    }
}
