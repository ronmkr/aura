use super::Event;
use crate::task::MetaTask;
use crate::{Result, TaskId};
use async_trait::async_trait;
use tokio::sync::broadcast;

/// Handles telemetry and event broadcasting from the engine.
pub trait EventSubscriber: Send + Sync {
    /// Returns a receiver for the global event bus.
    fn subscribe(&self) -> broadcast::Receiver<Event>;
}

/// Handles state mutations and lifecycle control for download tasks.
#[async_trait]
pub trait TaskController: Send + Sync {
    /// Pauses a task.
    async fn pause(&self, id: TaskId) -> Result<()>;

    /// Resumes a paused task.
    async fn resume(&self, id: TaskId) -> Result<()>;

    /// Removes a task from the engine.
    async fn remove(&self, id: TaskId) -> Result<()>;

    /// Changes options for an existing task.
    async fn change_option(
        &self,
        id: TaskId,
        priority: Option<u32>,
        depends_on: Option<Vec<TaskId>>,
        seed_ratio: Option<f32>,
        seed_time: Option<u32>,
        streaming_mode: Option<bool>,
    ) -> Result<()>;
}

/// Handles retrieving information and metadata for download tasks.
#[async_trait]
pub trait TaskQuerier: Send + Sync {
    /// Lists all active tasks currently managed by the engine.
    async fn tell_active(&self) -> Result<Vec<MetaTask>>;

    /// Returns the list of files for a specific task.
    async fn get_files(&self, id: TaskId) -> Result<Option<Vec<crate::torrent::File>>>;
}

/// A trait for components that can control task lifecycle and telemetry.
pub trait TaskHandleApi: EventSubscriber + TaskController + Send + Sync {}

impl<T> TaskHandleApi for T where T: EventSubscriber + TaskController + Send + Sync {}

/// A unified interface representing the full Engine API.
///
/// This combines the discrete traits into a single interface that can be
/// implemented by the concrete Engine or by test mocks.
pub trait EngineApi: EventSubscriber + TaskController + TaskQuerier + Send + Sync {}

impl<T> EngineApi for T where T: EventSubscriber + TaskController + TaskQuerier + Send + Sync {}
