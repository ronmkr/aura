use crate::TaskId;

/// Telemetry events published to the Event Bus.
#[derive(Debug, Clone, serde::Serialize)]
pub enum Event {
    TaskAdded(TaskId),
    MetadataResolved {
        id: TaskId,
        final_uri: String,
        total_length: u64,
        name: Option<String>,
    },
    TaskProgress {
        id: TaskId,
        completed_bytes: u64,
        uploaded_bytes: u64,
        total_bytes: u64,
    },
    TaskCompleted(TaskId),
    TaskPaused(TaskId),
    TaskResumed(TaskId),
    TaskError {
        id: TaskId,
        message: String,
    },
    SeedingComplete {
        id: TaskId,
        reason: crate::SeedingCompleteReason,
    },
}
