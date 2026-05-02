use super::{Command, Event, Orchestrator};
use crate::task::{DownloadPhase, MetaTask};
use crate::{Error, Result, TaskId};
use tracing::info;

impl Orchestrator {
    pub(crate) async fn handle_command(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::AddTask { id, name, sources } => {
                self.handle_add_task(id, name, sources).await?;
            }
            Command::Pause(id) => {
                self.handle_pause(id).await?;
            }
            Command::Resume(id) => {
                self.handle_resume(id).await?;
            }
            Command::Remove(id) => {
                let _ = self.handle_pause(id).await;
                self.tasks.remove(&id);
            }
            Command::ListActive(reply_tx) => {
                let active: Vec<MetaTask> = self.tasks.values().cloned().collect();
                let _ = reply_tx.send(active).await;
            }
            Command::ReloadConfig(new_config) => {
                info!("Reloading configuration");
                self.throttler
                    .set_limit(new_config.bandwidth.global_download_limit);
                self.config.store(new_config);
            }
            Command::KillSwitch => {
                let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                for id in ids {
                    let _ = self.handle_pause(id).await;
                }
            }
            Command::Shutdown => {
                info!("Orchestrator shutting down");
                // We return Err to break the loop in run() or just a special signal
                // For now, run() will see the end of command_rx if we drop it,
                // but explicit shutdown is better.
                return Err(Error::Storage("Shutting down".to_string()));
            }
        }
        Ok(())
    }

    pub(crate) async fn handle_add_task(
        &mut self,
        id: TaskId,
        name: String,
        sources: Vec<(String, crate::task::TaskType)>,
    ) -> Result<()> {
        info!(%id, %name, "Adding MetaTask with {} sources", sources.len());

        let path = format!("{}.aura", name);
        let (mut meta_task, loaded_bitfield) = if let Ok(data) = tokio::fs::read(&path).await {
            match serde_json::from_slice::<crate::task::TaskState>(&data) {
                Ok(state) => {
                    info!(%id, "Resuming task from control file {}", path);
                    let bitfield = state.bitfield.clone();
                    let mut mt = MetaTask::from_state(state);
                    mt.id = id; // Update to the new ID
                    (mt, bitfield)
                }
                Err(e) => {
                    tracing::warn!(%id, "Failed to parse control file {}: {}. Starting fresh.", path, e);
                    (MetaTask::new(id, name, 0), None)
                }
            }
        } else {
            (MetaTask::new(id, name, 0), None)
        };

        if meta_task.subtasks.is_empty() {
            for (uri, ttype) in sources {
                meta_task.add_subtask(uri, ttype);
            }
        }

        let token = tokio_util::sync::CancellationToken::new();
        self.cancellation_tokens.insert(id, token.clone());

        self.tasks.insert(id, meta_task);
        self.start_task_loops_with_bitfield(id, token, loaded_bitfield)
            .await?;

        let _ = self.event_tx.send(Event::TaskAdded(id));
        Ok(())
    }

    pub(crate) async fn handle_pause(&mut self, id: TaskId) -> Result<()> {
        if let Some(task) = self.tasks.get_mut(&id) {
            if task.phase != DownloadPhase::Paused && task.phase != DownloadPhase::Complete {
                info!(%id, "Pausing task");
                task.phase = DownloadPhase::Paused;

                if let Some(token) = self.cancellation_tokens.remove(&id) {
                    token.cancel();
                }

                // Recycle in-flight ranges
                let in_flight = std::mem::take(&mut task.in_flight_ranges);
                for (_sub_id, range) in in_flight {
                    task.pending_ranges.push(range);
                }

                let _ = self.event_tx.send(Event::TaskProgress {
                    id,
                    completed_bytes: task.completed_length,
                    total_bytes: task.total_length,
                });
            }
        }
        Ok(())
    }

    pub(crate) async fn handle_resume(&mut self, id: TaskId) -> Result<()> {
        if let Some(task) = self.tasks.get_mut(&id) {
            if task.phase == DownloadPhase::Paused {
                info!(%id, "Resuming task");
                task.phase = DownloadPhase::Downloading;
                let token = tokio_util::sync::CancellationToken::new();
                self.cancellation_tokens.insert(id, token.clone());

                // For resume, we don't reload bitfield from file here,
                // we assume the in-memory one is correct or will be synced.
                self.start_task_loops_with_bitfield(id, token, None).await?;
            }
        }
        Ok(())
    }
}
