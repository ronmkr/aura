use crate::task::MetaTask;
use crate::TaskId;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Internal commands for the Orchestrator.
#[derive(Debug)]
pub enum Command {
    AddTask {
        id: TaskId,
        name: String,
        sources: Vec<(String, crate::task::TaskType)>,
        checksum: Option<crate::Checksum>,
        priority: u32,
        streaming_mode: bool,
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
