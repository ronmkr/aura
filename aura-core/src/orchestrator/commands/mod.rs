use super::structs::{Command, Orchestrator};
use crate::task::MetaTask;
use crate::{Error, Result, TaskId};
use tracing::info;

pub(crate) mod add;
pub(crate) mod config;
pub(crate) mod lifecycle;
pub(crate) mod retry;

impl Orchestrator {
    pub(crate) async fn handle_command(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::AddTask {
                id,
                name,
                sources,
                checksum,
                priority,
                streaming_mode,
            } => {
                self.handle_add_task(id, name, sources, checksum, priority, streaming_mode)
                    .await?;
            }
            Command::Pause(id) => {
                self.handle_pause(id).await?;
            }
            Command::Resume(id) => {
                self.handle_resume(id).await?;
            }
            Command::Remove(id) => {
                let _ = self.handle_pause(id).await;
                self.throttler.unregister_task(id).await;
                self.tasks.remove(&id);
            }
            Command::ListActive(reply_tx) => {
                let active: Vec<MetaTask> = self.tasks.values().cloned().collect();
                let _ = reply_tx.send(active).await;
            }
            Command::GetConfig(reply_tx) => {
                let config = self.config.load().clone();
                let _ = reply_tx.send(config).await;
            }
            Command::ReloadConfig(new_config, resp_tx) => {
                self.handle_reload_config(new_config, resp_tx).await;
            }
            Command::KillSwitch => {
                let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                for id in ids {
                    let _ = self.handle_pause(id).await;
                }
            }
            Command::Shutdown => {
                info!("Orchestrator shutting down");
                return Err(Error::Engine("Shutting down".to_string()));
            }
            Command::RetrySubtask(meta_id, sub_id) => {
                self.handle_retry_subtask(meta_id, sub_id).await?;
            }
        }
        Ok(())
    }
}
