use crate::orchestrator::{EngineApi, Event};
use crate::{Result, TaskId};
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::Stream;

/// High-level events for a specific download task.
#[derive(Debug, Clone, serde::Serialize)]
pub enum TaskEvent {
    /// Metadata (filename, size, etc.) has been resolved for the task.
    MetadataResolved {
        final_uri: String,
        total_length: u64,
        name: Option<String>,
    },
    /// Periodic progress update.
    Progress {
        completed_bytes: u64,
        uploaded_bytes: u64,
        total_bytes: u64,
    },
    /// The task has finished successfully and data is verified.
    Completed,
    /// An error occurred during the download process.
    Error(String),
    /// Seeding limit reached.
    SeedingComplete {
        reason: crate::SeedingCompleteReason,
    },
}

/// A handle to an active or pending download task.
///
/// `TaskHandle` is cloneable and can be shared across threads. It provides
/// methods to control the task lifecycle and subscribe to its telemetry stream.
#[derive(Clone)]
pub struct TaskHandle {
    id: TaskId,
    engine: Arc<dyn EngineApi>,
}

impl TaskHandle {
    pub(crate) fn new(id: TaskId, engine: Arc<dyn EngineApi>) -> Self {
        Self { id, engine }
    }

    /// Returns the unique identifier for this task.
    pub fn id(&self) -> TaskId {
        self.id
    }

    /// Subscribes to a stream of telemetry events for this task.
    ///
    /// This stream filters the global event bus and only emits events related to
    /// this specific task.
    pub fn events(&self) -> Pin<Box<dyn Stream<Item = TaskEvent> + Send>> {
        let id = self.id;
        let mut rx = self.engine.subscribe();

        let stream = async_stream::stream! {
            while let Ok(event) = rx.recv().await {
                match event {
                    Event::MetadataResolved {
                        id: ev_id,
                        final_uri,
                        total_length,
                        name,
                    } if ev_id == id => yield TaskEvent::MetadataResolved {
                        final_uri,
                        total_length,
                        name,
                    },
                    Event::TaskProgress {
                        id: ev_id,
                        completed_bytes,
                        uploaded_bytes,
                        total_bytes,
                    } if ev_id == id => yield TaskEvent::Progress {
                        completed_bytes,
                        uploaded_bytes,
                        total_bytes,
                    },
                    Event::TaskCompleted(ev_id) if ev_id == id => yield TaskEvent::Completed,
                    Event::TaskError { id: ev_id, message } if ev_id == id => {
                        yield TaskEvent::Error(message)
                    }
                    Event::SeedingComplete { id: ev_id, reason } if ev_id == id => {
                        yield TaskEvent::SeedingComplete { reason }
                    }
                    _ => {}
                }
            }
        };

        Box::pin(stream)
    }

    /// Pauses the download task.
    pub async fn pause(&self) -> Result<()> {
        self.engine.pause(self.id).await
    }

    /// Resumes a paused download task.
    pub async fn resume(&self) -> Result<()> {
        self.engine.resume(self.id).await
    }

    /// Removes the task from the engine and deletes its control file.
    /// Note: This does not delete the downloaded data.
    pub async fn remove(&self) -> Result<()> {
        self.engine.remove(self.id).await
    }
}
