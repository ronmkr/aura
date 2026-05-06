//! api: Ergonomic public API for managing download tasks.
//!
//! This module provides the primary interface for embedding Aura into other applications,
//! as defined in ADR 0020. It uses an asynchronous, stream-based model for telemetry
//! and provides lightweight handles for task control.

use crate::orchestrator::{Engine, Event};
use crate::{Result, TaskId};
use std::pin::Pin;
use tokio_stream::{Stream, StreamExt};

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
    /// Creates a new `TaskHandle`. Internal use only.
    pub(crate) fn new(id: TaskId, engine: Engine) -> Self {
        Self { id, engine }
    }

    /// Returns the unique identifier of the task.
    pub fn id(&self) -> TaskId {
        self.id
    }

    /// Pauses the download task.
    pub async fn pause(&self) -> Result<()> {
        self.engine.pause(self.id).await
    }

    /// Resumes a paused download task.
    pub async fn resume(&self) -> Result<()> {
        self.engine.unpause(self.id).await
    }

    /// Cancels and removes the download task.
    pub async fn remove(&self) -> Result<()> {
        self.engine.remove(self.id).await
    }

    /// Returns a stream of events for this specific task.
    ///
    /// This stream filters the global event bus and only emits events related to
    /// this task's ID.
    ///
    /// # Example
    /// ```no_run
    /// # use aura_core::TaskHandle;
    /// # use tokio_stream::StreamExt;
    /// # async fn example(handle: TaskHandle) {
    /// let mut events = handle.events();
    /// while let Some(event) = events.next().await {
    ///     println!("Received event: {:?}", event);
    /// }
    /// # }
    /// ```
    pub fn events(&self) -> Pin<Box<dyn Stream<Item = TaskEvent> + Send>> {
        let id = self.id;
        let rx = self.engine.subscribe();

        let stream =
            tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(move |res| match res {
                Ok(event) => match event {
                    Event::MetadataResolved {
                        id: ev_id,
                        final_uri,
                        total_length,
                        name,
                    } if ev_id == id => Some(TaskEvent::MetadataResolved {
                        final_uri,
                        total_length,
                        name,
                    }),
                    Event::TaskProgress {
                        id: ev_id,
                        completed_bytes,
                        total_bytes,
                    } if ev_id == id => Some(TaskEvent::Progress {
                        completed_bytes,
                        total_bytes,
                    }),
                    Event::TaskCompleted(ev_id) if ev_id == id => Some(TaskEvent::Completed),
                    Event::TaskError { id: ev_id, message } if ev_id == id => {
                        Some(TaskEvent::Error(message))
                    }
                    _ => None,
                },
                Err(_) => None,
            });

        Box::pin(stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::Event;
    use proptest::prelude::*;
    use tokio::sync::broadcast;
    use tokio_stream::StreamExt;

    proptest! {
        #[test]
        fn test_task_event_filtering_proptest(id_val in 0u64..100u64, other_id_val in 0u64..100u64) {
            let (event_tx, _event_rx) = broadcast::channel(100);
            let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);

            let engine = Engine {
                command_tx,
                event_tx: event_tx.clone(),
            };

            let id = TaskId(id_val);
            let handle = TaskHandle::new(id, engine);

            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            let mut events = handle.events();

            rt.block_on(async {
                // Send event for target ID
                event_tx.send(Event::TaskCompleted(id)).unwrap();

                // Send event for other ID
                if id_val != other_id_val {
                    event_tx.send(Event::TaskCompleted(TaskId(other_id_val))).unwrap();
                }

                // Verify target event received
                let e = events.next().await.unwrap();
                assert!(matches!(e, TaskEvent::Completed));

                // Verify no more events (filtering works)
                tokio::select! {
                    _ = events.next() => panic!("Should have filtered other event"),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {}
                }
            });
        }
    }

    #[tokio::test]
    async fn test_task_handle_events() {
        let (event_tx, _event_rx) = broadcast::channel(10);
        let (command_tx, _command_rx) = tokio::sync::mpsc::channel(1);

        let engine = Engine {
            command_tx,
            event_tx: event_tx.clone(),
        };

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
                total_bytes: 1000,
            })
            .unwrap();

        // Emit event for ANOTHER task (should be filtered out)
        event_tx
            .send(Event::TaskProgress {
                id: TaskId(456),
                completed_bytes: 100,
                total_bytes: 1000,
            })
            .unwrap();

        event_tx.send(Event::TaskCompleted(id)).unwrap();

        // Verify events
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
}
