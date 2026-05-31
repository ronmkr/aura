use crate::task::MetaTask;
use crate::{TaskId, TenantId};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Internal commands for the Orchestrator.
#[derive(Debug)]
pub enum Command {
    AddTask {
        id: TaskId,
        tenant_id: Option<TenantId>,
        name: String,
        sources: Vec<(String, crate::task::TaskType)>,
        checksum: Option<crate::Checksum>,
        priority: u32,
        streaming_mode: bool,
        depends_on: Vec<TaskId>,
        follow_on: Option<crate::task::FollowOnAction>,
    },
    ChangeOption {
        id: TaskId,
        priority: Option<u32>,
        depends_on: Option<Vec<TaskId>>,
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
}
