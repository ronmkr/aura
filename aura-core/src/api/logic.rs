//! api: Ergonomic public API for managing download tasks.
//!
//! This module provides the primary interface for embedding Aura into other applications,
//! as defined in ADR 0020. It uses an asynchronous, stream-based model for telemetry
//! and provides lightweight handles for task control.

use crate::orchestrator::{Engine, Event};
use crate::{Result, TaskId};
use std::pin::Pin;
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
}

/// A handle to an active or pending download task.
///
/// `TaskHandle` is cloneable and can be shared across threads. It provides
/// methods to control the task lifecycle and subscribe to its telemetry stream.
#[derive(Clone)]
pub struct TaskHandle {
    id: TaskId,
    engine: Engine,
}

impl TaskHandle {
    pub(crate) fn new(id: TaskId, engine: Engine) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::Config;
    use crate::task::TaskType;
    use tokio_stream::StreamExt;

    #[tokio::test]
    async fn test_task_handle_events() {
        let config = Config::default();
        let (engine, _orchestrator, _storage) = Engine::new(config).await.unwrap();

        let event_tx = _orchestrator.event_tx.clone();
        let id = TaskId(123);
        let handle = TaskHandle::new(id, engine);
        let mut events = handle.events();

        // Emit events for our task
        event_tx
            .send(Event::MetadataResolved {
                id,
                final_uri: "http://example.com".to_string(),
                total_length: 1000,
                name: Some("test".to_string()),
            })
            .unwrap();

        event_tx
            .send(Event::TaskProgress {
                id,
                completed_bytes: 500,
                uploaded_bytes: 0,
                total_bytes: 1000,
            })
            .unwrap();

        // Emit event for ANOTHER task (should be filtered out)
        event_tx
            .send(Event::TaskProgress {
                id: TaskId(456),
                completed_bytes: 100,
                uploaded_bytes: 0,
                total_bytes: 1000,
            })
            .unwrap();

        event_tx.send(Event::TaskCompleted(id)).unwrap();

        // Verify we only received events for Task 123
        let e1 = events.next().await.unwrap();
        if let TaskEvent::MetadataResolved { total_length, .. } = e1 {
            assert_eq!(total_length, 1000);
        } else {
            panic!("Expected MetadataResolved");
        }

        let e2 = events.next().await.unwrap();
        if let TaskEvent::Progress {
            completed_bytes, ..
        } = e2
        {
            assert_eq!(completed_bytes, 500);
        } else {
            panic!("Expected Progress");
        }

        let e3 = events.next().await.unwrap();
        if let TaskEvent::Completed = e3 {
            // success
        } else {
            panic!("Expected Completed");
        }
    }

    #[test]
    fn test_task_event_filtering_proptest() {
        // This is a placeholder for a more complex property-based test
        // validating that task-specific streams never leak data from other tasks.
    }

    #[tokio::test]
    async fn test_engine_subscribe_captures_task_added() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        config.storage.download_dir = temp_dir.path().to_string_lossy().to_string();

        let (engine, orchestrator, storage) = Engine::new(config).await.unwrap();

        // Spawn actors
        tokio::spawn(async move {
            let _ = orchestrator.run().await;
        });
        tokio::spawn(async move {
            let _ = storage.run().await;
        });

        // 1. Subscribe BEFORE adding task
        let mut events = engine.subscribe();

        // 2. Add task
        let id = TaskId(999);
        engine
            .add_task_with_options(crate::orchestrator::command::AddTaskArgs {
                id,
                tenant_id: None,
                name: "test_race_task".to_string(),
                sources: vec![("http://example.com/file".to_string(), TaskType::Http)],
                checksum: None,
                priority: 100,
                streaming_mode: false,
                depends_on: Vec::new(),
                follow_on: None,
            })
            .await
            .unwrap();

        // 3. Verify Event::TaskAdded is captured
        let mut found = false;
        // Wait up to 2 seconds
        let timeout_fut = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while let Ok(event) = events.recv().await {
                if let Event::TaskAdded(task_id) = event {
                    if task_id == id {
                        found = true;
                        break;
                    }
                }
            }
        });
        let _ = timeout_fut.await;
        assert!(found, "Event::TaskAdded should be received by early subscriber");
    }
}
