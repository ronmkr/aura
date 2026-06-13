use crate::bitfield::Bitfield;
use crate::{TaskId, TenantId};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::meta::MetaTask;
use super::phase::{DownloadPhase, FollowOnAction};
use super::range::Range;
use super::subtask::SubTask;

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
            seed_ratio_override: None,
            seed_time_override: None,
        }
    }
}
