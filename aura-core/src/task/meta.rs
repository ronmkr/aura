use super::extension::TaskExtension;
use super::phase::{DownloadPhase, FollowOnAction, TaskType};
use super::range::Range;
use super::subtask::SubTask;
use crate::bitfield::Bitfield;
use crate::{TaskId, TenantId};
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
    pub seed_ratio_override: Option<f32>,
    pub seed_time_override: Option<u32>,
}

impl MetaTask {
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
            seed_ratio_override: None,
            seed_time_override: None,
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

    fn bt_task(&self) -> Option<&crate::worker::bittorrent::task::BtTask> {
        self.extensions
            .get(crate::worker::bittorrent::BT_EXTENSION_KEY)?
            .as_any()
            .downcast_ref::<crate::worker::bittorrent::task::BtTask>()
    }

    pub fn uploaded_length(&self) -> u64 {
        self.bt_task()
            .map(|bt| {
                bt.state
                    .uploaded_length
                    .load(std::sync::atomic::Ordering::Relaxed)
            })
            .unwrap_or(0)
    }

    pub fn seed_ratio(&self) -> Option<f32> {
        self.bt_task()
            .and_then(|bt| *bt.state.seed_ratio.lock().unwrap())
    }

    pub fn seed_time(&self) -> Option<u32> {
        self.bt_task()
            .and_then(|bt| *bt.state.seed_time.lock().unwrap())
    }

    pub fn swarm_seeders(&self) -> Option<u32> {
        self.bt_task().map(|bt| {
            bt.state
                .swarm_seeders
                .load(std::sync::atomic::Ordering::Relaxed)
        })
    }

    pub fn swarm_leechers(&self) -> Option<u32> {
        self.bt_task().map(|bt| {
            bt.state
                .swarm_leechers
                .load(std::sync::atomic::Ordering::Relaxed)
        })
    }
}
