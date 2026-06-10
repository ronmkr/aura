use crate::task::{FollowOnAction, MetaTask, TaskType};
use crate::{Checksum, TaskId, TenantId};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Arguments for adding a new task to the engine.
#[derive(Debug, Clone)]
pub struct AddTaskArgs {
    pub id: TaskId,
    pub tenant_id: Option<TenantId>,
    pub name: String,
    pub sources: Vec<(String, TaskType)>,
    pub checksum: Option<Checksum>,
    pub priority: u32,
    pub streaming_mode: bool,
    pub depends_on: Vec<TaskId>,
    pub follow_on: Option<FollowOnAction>,
}

/// Internal commands for the Orchestrator.
#[derive(Debug)]
pub enum Command {
    AddTask(AddTaskArgs),
    ChangeOption {
        id: TaskId,
        priority: Option<u32>,
        depends_on: Option<Vec<TaskId>>,
        seed_ratio: Option<f32>,
        seed_time: Option<u32>,
        streaming_mode: Option<bool>,
    },
    Pause(TaskId),
    Resume(TaskId),
    Remove(TaskId),
    ListActive(mpsc::Sender<Vec<MetaTask>>),
    GetConfig(mpsc::Sender<Arc<crate::Config>>),
    ReloadConfig(Arc<crate::Config>, tokio::sync::oneshot::Sender<()>),
    KillSwitch,
    Shutdown,
    RetrySubtask(TaskId, TaskId),
    Scrub(TaskId),
    RefreshDiscovery(TaskId),
    Refresh(TaskId),
    GetFiles(
        TaskId,
        tokio::sync::oneshot::Sender<Option<Vec<crate::torrent::File>>>,
    ),
    SetFileSelection(TaskId, Vec<bool>),
}
