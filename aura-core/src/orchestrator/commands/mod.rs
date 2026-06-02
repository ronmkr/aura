use super::{Command, Orchestrator};
use crate::task::MetaTask;
use crate::worker::bittorrent::task::BtTask;
use crate::{Error, Result, TaskId};
use tracing::info;

pub(crate) mod add;
pub(crate) mod config;
pub(crate) mod dependency;
pub(crate) mod lifecycle;
pub(crate) mod retry;

impl Orchestrator {
    pub(crate) async fn handle_command(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::AddTask(args) => {
                self.handle_add_task(args).await?;
            }
            Command::ChangeOption {
                id,
                priority,
                depends_on,
            } => {
                self.handle_change_option(id, priority, depends_on).await?;
            }
            Command::Pause(id) => {
                self.handle_pause(id).await?;
            }
            Command::Resume(id) => {
                self.handle_resume(id).await?;
            }
            Command::Remove(id) => {
                let _ = self.handle_pause(id).await;
                let throttler = if let Some(task) = self.tasks.get(&id) {
                    if let Some(ref tid) = task.tenant_id {
                        if let Some(ctx) = self.tenants.get(tid) {
                            ctx.throttler.clone()
                        } else {
                            self.throttler.clone()
                        }
                    } else {
                        self.throttler.clone()
                    }
                } else {
                    self.throttler.clone()
                };
                throttler.unregister_task(id).await;
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
                        // BitTorrent tasks store their protocol-specific state as an extension in the MetaTask.
                        if let Some(_bt_sub) = meta_task
                            .subtasks
                            .iter()
                            .find(|s| s.task_type == crate::task::TaskType::BitTorrent)
                        {
                            if let Some(bt_task) = meta_task
                                .extensions
                                .get("bittorrent")
                                .and_then(|e| e.clone().as_any_arc().downcast::<BtTask>().ok())
                            {
                                let config = self.config.load();
                                let base_dir: std::path::PathBuf = if let Some(ref tid) =
                                    meta_task.tenant_id
                                {
                                    if let Some(ctx) = self.tenants.get(tid) {
                                        if let Some(ref root) = ctx.disk_path_root {
                                            root.clone()
                                        } else {
                                            std::path::PathBuf::from(&config.storage.download_dir)
                                        }
                                    } else {
                                        std::path::PathBuf::from(&config.storage.download_dir)
                                    }
                                } else {
                                    std::path::PathBuf::from(&config.storage.download_dir)
                                };
                                let path = base_dir.join(&meta_task.name);
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
                        let base_dir: std::path::PathBuf =
                            if let Some(ref tid) = meta_task.tenant_id {
                                if let Some(ctx) = self.tenants.get(tid) {
                                    if let Some(ref root) = ctx.disk_path_root {
                                        root.clone()
                                    } else {
                                        std::path::PathBuf::from(&config.storage.download_dir)
                                    }
                                } else {
                                    std::path::PathBuf::from(&config.storage.download_dir)
                                }
                            } else {
                                std::path::PathBuf::from(&config.storage.download_dir)
                            };
                        let path = base_dir.join(&meta_task.name);
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
                    if let Some(_bt_sub) = meta_task
                        .subtasks
                        .iter()
                        .find(|s| s.task_type == crate::task::TaskType::BitTorrent)
                    {
                        if let Some(bt_task) = meta_task
                            .extensions
                            .get("bittorrent")
                            .and_then(|e| e.clone().as_any_arc().downcast::<BtTask>().ok())
                        {
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
