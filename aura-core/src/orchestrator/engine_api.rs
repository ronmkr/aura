use super::{Command, Engine, Event};
use crate::task::{MetaTask, TaskType};
use crate::{Error, Result, TaskId};
use rand::RngExt;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

impl Engine {
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    pub async fn add_task(
        &self,
        name: String,
        uri: String,
        task_type: TaskType,
    ) -> Result<crate::api::TaskHandle> {
        let id = TaskId(rand::rng().random());
        self.add_task_with_sources(id, None, name, vec![(uri, task_type)], None)
            .await
    }

    pub async fn add_task_with_id(
        &self,
        id: TaskId,
        name: String,
        uri: String,
        task_type: TaskType,
    ) -> Result<crate::api::TaskHandle> {
        self.add_task_with_sources(id, None, name, vec![(uri, task_type)], None)
            .await
    }

    pub async fn add_task_with_checksum(
        &self,
        id: TaskId,
        name: String,
        uri: String,
        task_type: TaskType,
        checksum: Option<crate::Checksum>,
    ) -> Result<crate::api::TaskHandle> {
        self.add_task_with_sources(id, None, name, vec![(uri, task_type)], checksum)
            .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn add_task_with_options(
        &self,
        id: TaskId,
        tenant_id: Option<crate::TenantId>,
        name: String,
        sources: Vec<(String, TaskType)>,
        checksum: Option<crate::Checksum>,
        priority: u32,
        streaming_mode: bool,
        depends_on: Vec<TaskId>,
    ) -> Result<crate::api::TaskHandle> {
        self.command_tx
            .send(Command::AddTask {
                id,
                tenant_id,
                name,
                sources,
                checksum,
                priority,
                streaming_mode,
                depends_on,
            })
            .await
            .map_err(|e| Error::Engine(format!("Failed to send AddTask command: {}", e)))?;
        Ok(crate::api::TaskHandle::new(id, self.clone()))
    }

    pub async fn add_task_with_sources(
        &self,
        id: TaskId,
        tenant_id: Option<crate::TenantId>,
        name: String,
        sources: Vec<(String, TaskType)>,
        checksum: Option<crate::Checksum>,
    ) -> Result<crate::api::TaskHandle> {
        self.add_task_with_options(
            id,
            tenant_id,
            name,
            sources,
            checksum,
            100,
            false,
            Vec::new(),
        )
        .await
    }

    pub async fn change_option(
        &self,
        id: TaskId,
        priority: Option<u32>,
        depends_on: Option<Vec<TaskId>>,
    ) -> Result<()> {
        self.command_tx
            .send(Command::ChangeOption {
                id,
                priority,
                depends_on,
            })
            .await
            .map_err(|e| Error::Engine(format!("Failed to send ChangeOption command: {}", e)))?;
        Ok(())
    }

    pub async fn tell_active(&self) -> Result<Vec<MetaTask>> {
        let (tx, mut rx) = mpsc::channel(1);
        self.command_tx
            .send(Command::ListActive(tx))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send ListActive command: {}", e)))?;
        rx.recv()
            .await
            .ok_or_else(|| Error::Engine("Engine shut down".to_string()))
    }

    pub async fn pause(&self, id: TaskId) -> Result<()> {
        self.command_tx
            .send(Command::Pause(id))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send Pause command: {}", e)))?;
        Ok(())
    }

    pub async fn resume(&self, id: TaskId) -> Result<()> {
        self.command_tx
            .send(Command::Resume(id))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send Resume command: {}", e)))?;
        Ok(())
    }

    pub async fn load_tasks_from_dir(&self, dir: &str) -> Result<()> {
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| Error::Engine(format!("Failed to read dir {}: {}", dir, e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::Engine(format!("Failed to read next entry in {}: {}", dir, e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("aura") {
                let data = tokio::fs::read(&path).await.map_err(|e| {
                    Error::Engine(format!("Failed to read control file {:?}: {}", path, e))
                })?;
                let state: crate::task::TaskState = serde_json::from_slice(&data).map_err(|e| {
                    Error::Engine(format!(
                        "Failed to deserialize task state from {:?}: {}",
                        path, e
                    ))
                })?;

                info!("Found persisted task: {}", state.name);
                // For now we just add it back with original sources
                let sources = state
                    .subtasks
                    .iter()
                    .map(|s| (s.uri.clone(), s.task_type.clone()))
                    .collect();
                let id = state.id;
                let checksum = state.checksum.clone();
                let _ = self
                    .add_task_with_options(
                        id,
                        state.tenant_id,
                        state.name,
                        sources,
                        checksum,
                        state.priority,
                        state.streaming_mode,
                        state.depends_on.unwrap_or_default(),
                    )
                    .await;
            }
        }
        Ok(())
    }

    pub async fn remove(&self, id: TaskId) -> Result<()> {
        self.command_tx
            .send(Command::Remove(id))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send Remove command: {}", e)))?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.command_tx
            .send(Command::Shutdown)
            .await
            .map_err(|e| Error::Engine(format!("Failed to send Shutdown command: {}", e)))?;
        Ok(())
    }

    pub async fn trigger_killswitch(&self) -> Result<()> {
        self.command_tx
            .send(Command::KillSwitch)
            .await
            .map_err(|e| Error::Engine(format!("Failed to send KillSwitch command: {}", e)))?;
        Ok(())
    }

    pub async fn reload_config(&self, config: crate::Config) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.command_tx
            .send(Command::ReloadConfig(Arc::new(config), tx))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send ReloadConfig command: {}", e)))?;

        let _ = rx.await;
        Ok(())
    }

    pub async fn tell_config(&self) -> Result<Arc<crate::Config>> {
        let (tx, mut rx) = mpsc::channel(1);
        self.command_tx
            .send(Command::GetConfig(tx))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send GetConfig command: {}", e)))?;
        rx.recv()
            .await
            .ok_or_else(|| Error::Engine("Engine shut down".to_string()))
    }
}
