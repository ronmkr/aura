use super::{Command, Engine, Event};
use crate::orchestrator::TaskHandle;
use crate::task::{MetaTask, TaskType};
use crate::{Error, Result, TaskId};
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
    ) -> Result<TaskHandle> {
        let id = TaskId::random();
        self.add_task_with_sources(id, None, name, vec![(uri, task_type)], None)
            .await
    }

    pub async fn add_task_with_id(
        &self,
        id: TaskId,
        name: String,
        uri: String,
        task_type: TaskType,
    ) -> Result<TaskHandle> {
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
    ) -> Result<TaskHandle> {
        self.add_task_with_sources(id, None, name, vec![(uri, task_type)], checksum)
            .await
    }

    pub async fn add_task_with_options(
        &self,
        args: crate::orchestrator::command::AddTaskArgs,
    ) -> Result<TaskHandle> {
        let id = args.id;
        self.command_tx
            .send(Command::AddTask(args))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send AddTask command: {}", e)))?;
        Ok(TaskHandle::new(id, self.clone()))
    }

    pub async fn add_task_with_sources(
        &self,
        id: TaskId,
        tenant_id: Option<crate::TenantId>,
        name: String,
        sources: Vec<(String, TaskType)>,
        checksum: Option<crate::Checksum>,
    ) -> Result<TaskHandle> {
        let config = self.config.load();
        self.add_task_with_options(crate::orchestrator::command::AddTaskArgs {
            id,
            tenant_id,
            name,
            sources,
            checksum,
            priority: config.limits.default_task_priority,
            streaming_mode: false,
            depends_on: Vec::new(),
            follow_on: None,
        })
        .await
    }

    pub async fn change_option(
        &self,
        id: TaskId,
        priority: Option<u32>,
        depends_on: Option<Vec<TaskId>>,
        seed_ratio: Option<f32>,
        seed_time: Option<u32>,
        streaming_mode: Option<bool>,
    ) -> Result<()> {
        self.command_tx
            .send(Command::ChangeOption {
                id,
                priority,
                depends_on,
                seed_ratio,
                seed_time,
                streaming_mode,
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

    pub async fn refresh(&self, id: TaskId) -> Result<()> {
        self.command_tx
            .send(Command::Refresh(id))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send Refresh command: {}", e)))?;
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
                self.add_task_with_options(crate::orchestrator::command::AddTaskArgs {
                    id,
                    tenant_id: state.tenant_id,
                    name: state.name,
                    sources,
                    checksum,
                    priority: state.priority,
                    streaming_mode: state.streaming_mode,
                    depends_on: state.depends_on.unwrap_or_default(),
                    follow_on: state.follow_on,
                })
                .await?;
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

    pub async fn reload_config(&self, config: crate::AuraConfig) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.command_tx
            .send(Command::ReloadConfig(Arc::new(config), tx))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send ReloadConfig command: {}", e)))?;

        let _ = rx.await;
        Ok(())
    }

    pub async fn tell_config(&self) -> Result<Arc<crate::AuraConfig>> {
        let (tx, mut rx) = mpsc::channel(1);
        self.command_tx
            .send(Command::GetConfig(tx))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send GetConfig command: {}", e)))?;
        rx.recv()
            .await
            .ok_or_else(|| Error::Engine("Engine shut down".to_string()))
    }

    pub async fn tell_history(
        &self,
        offset: usize,
        num: usize,
    ) -> Result<Vec<crate::history::CompletedTaskRecord>> {
        let config = self.config.load();
        let mut records = crate::history::HistoryManager::read_records(&config);
        records.reverse();
        let paginated = records.into_iter().skip(offset).take(num).collect();
        Ok(paginated)
    }

    pub async fn get_files(&self, id: TaskId) -> Result<Option<Vec<crate::torrent::File>>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.command_tx
            .send(Command::GetFiles(id, tx))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send GetFiles command: {}", e)))?;
        rx.await
            .map_err(|_| Error::Engine("Engine shut down before replying".to_string()))
    }

    pub async fn set_file_selection(&self, id: TaskId, selection: Vec<bool>) -> Result<()> {
        self.command_tx
            .send(Command::SetFileSelection(id, selection))
            .await
            .map_err(|e| {
                Error::Engine(format!("Failed to send SetFileSelection command: {}", e))
            })?;
        Ok(())
    }

    pub async fn force_recheck(&self, id: TaskId) -> Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel::<Result<()>>();
        self.command_tx
            .send(Command::ForceRecheck(id, tx))
            .await
            .map_err(|e| Error::Engine(format!("Failed to send ForceRecheck command: {}", e)))?;
        rx.await
            .map_err(|_| Error::Engine("Engine shut down before replying".to_string()))?
    }
}
