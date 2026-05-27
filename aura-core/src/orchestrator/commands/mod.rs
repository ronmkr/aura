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
            Command::Scrub(id) => {
                if let Some(meta_task) = self.tasks.get(&id) {
                    if meta_task
                        .subtasks
                        .iter()
                        .any(|s| s.task_type == crate::task::TaskType::BitTorrent)
                    {
                        // For BitTorrent tasks, we might not have a direct mapping from meta_id to bt_task yet.
                        // But wait, the BtRegistry maps InfoHash to meta_id? Actually, BtTasks are keyed by sub_id in `self.bt_tasks`.
                        // Let's find the sub_id of the BitTorrent task.
                        if let Some(bt_sub) = meta_task
                            .subtasks
                            .iter()
                            .find(|s| s.task_type == crate::task::TaskType::BitTorrent)
                        {
                            if let Some(bt_task) = self.bt_tasks.get(&bt_sub.id) {
                                let config = self.config.load();
                                let path = std::path::Path::new(&config.storage.download_dir)
                                    .join(&meta_task.name);
                                let _ = self
                                    .scrub_tx
                                    .send(crate::scrubber::ScrubberCommand::ScrubSwarm {
                                        task_id: id,
                                        path,
                                        bt_task: bt_task.clone(),
                                    })
                                    .await;
                            }
                        }
                    } else if let Some(checksum) = meta_task.checksum.clone() {
                        let config = self.config.load();
                        let path = std::path::Path::new(&config.storage.download_dir)
                            .join(&meta_task.name);
                        let _ = self
                            .scrub_tx
                            .send(crate::scrubber::ScrubberCommand::ScrubNonSwarm {
                                task_id: id,
                                path,
                                checksum,
                            })
                            .await;
                    }
                }
            }
            Command::RefreshDiscovery(id) => {
                if let Some(meta_task) = self.tasks.get(&id) {
                    if let Some(bt_sub) = meta_task
                        .subtasks
                        .iter()
                        .find(|s| s.task_type == crate::task::TaskType::BitTorrent)
                    {
                        if let Some(bt_task) = self.bt_tasks.get(&bt_sub.id) {
                            let info_hash = bt_task.state.info_hash;
                            let _ = self
                                .dht_tx
                                .send(crate::dht::DhtCommand::Announce {
                                    info_hash,
                                    port: 6881,
                                })
                                .await;
                            let _ = self
                                .lpd_tx
                                .send(crate::lpd::LpdCommand::Announce {
                                    info_hash,
                                    port: 6881,
                                })
                                .await;
                            tracing::info!(%id, "Refreshed peer discovery via DHT and LPD");
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
