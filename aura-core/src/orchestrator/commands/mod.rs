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
                seed_ratio,
                seed_time,
            } => {
                self.handle_change_option(id, priority, depends_on, seed_ratio, seed_time)
                    .await?;
            }
            Command::Pause(id) => {
                self.handle_pause(id).await?;
            }
            Command::Refresh(id) => {
                self.handle_refresh(id).await?;
            }
            Command::Resume(id) => {
                self.handle_resume(id).await?;
            }
            Command::Remove(id) => {
                let _ = self.handle_pause(id).await;
                if let Some(task) = self.tasks.get(&id) {
                    if task.phase != crate::task::DownloadPhase::Complete
                        && task.phase != crate::task::DownloadPhase::Error
                    {
                        let duration_secs = task
                            .created_at
                            .map(|t| (chrono::Utc::now() - t).num_seconds().max(0) as u64)
                            .unwrap_or(0);
                        let record = crate::history::CompletedTaskRecord {
                            id: task.id.0.to_string(),
                            name: task.name.clone(),
                            uris: task.subtasks.iter().map(|s| s.uri.clone()).collect(),
                            total_bytes: task.total_length,
                            downloaded_bytes: task.completed_length,
                            uploaded_bytes: task.uploaded_length(),
                            duration_secs,
                            checksum_verified: Some(false),
                            phase: "Removed".to_string(),
                            error: Some("Task removed by user".to_string()),
                            completed_at: chrono::Utc::now(),
                        };
                        let config = self.config.load();
                        crate::history::HistoryManager::append_record(&config, record);
                    }
                }
                let throttler = if let Some(task) = self.tasks.get(&id) {
                    self.resolve_throttler(&task.tenant_id)
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

                // Explicitly flush DHT routing table during shutdown (ADR-0017)
                let (dht_save_tx, dht_save_rx) = tokio::sync::oneshot::channel();
                if let Err(e) = self
                    .dht_tx
                    .send(crate::dht::DhtCommand::SaveNow(dht_save_tx))
                    .await
                {
                    tracing::warn!("Failed to send SaveNow command to DHT actor: {}", e);
                } else {
                    // Await acknowledgement with a short timeout
                    match tokio::time::timeout(std::time::Duration::from_millis(500), dht_save_rx)
                        .await
                    {
                        Ok(Ok(())) => {
                            info!("DHT routing table successfully flushed on shutdown");
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(
                                "DHT actor closed channel before SaveNow completed: {}",
                                e
                            );
                        }
                        Err(_) => {
                            tracing::warn!(
                                "Timeout waiting for DHT routing table flush on shutdown"
                            );
                        }
                    }
                }

                // Send Stopped event announcements to all active trackers during shutdown (ADR-0058 Edge Case 3).
                let config = self.config.load();
                let port = config.network.listen_port;
                let local_addr = config.network.local_addr;
                let user_agent = Some(config.network.user_agent.clone());
                let proxy = config.network.proxy.clone();
                let tracker = std::sync::Arc::new(crate::tracker::TrackerClient::new(
                    self.peer_id,
                    port,
                    local_addr,
                    user_agent,
                    proxy,
                    Some(self.config.clone()),
                ));

                let mut announce_futures = Vec::new();
                for meta_task in self.tasks.values() {
                    if let Some(ext) = meta_task
                        .extensions
                        .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
                    {
                        if let Ok(bt_task) = ext.clone().as_any_arc().downcast::<BtTask>() {
                            if let Some(torrent) = bt_task.state.torrent.lock().await.clone() {
                                let tracker_clone = std::sync::Arc::clone(&tracker);
                                announce_futures.push(async move {
                                    let _ = tracker_clone.announce_stopped(&torrent).await;
                                });
                            }
                        }
                    }
                }

                if !announce_futures.is_empty() {
                    info!(
                        "Sending stopped announcements to trackers for {} torrents...",
                        announce_futures.len()
                    );
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(2),
                        futures_util::future::join_all(announce_futures),
                    )
                    .await;
                }

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
                                .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
                                .and_then(|e| e.clone().as_any_arc().downcast::<BtTask>().ok())
                            {
                                let base_dir = self.resolve_base_dir(&meta_task.tenant_id);
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
                        let base_dir = self.resolve_base_dir(&meta_task.tenant_id);
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
                            .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
                            .and_then(|e| e.clone().as_any_arc().downcast::<BtTask>().ok())
                        {
                            let info_hash = bt_task.state.info_hash;
                            let config = self.config.load();
                            let port = config.network.listen_port;
                            let _ = self
                                .dht_tx
                                .send(crate::dht::DhtCommand::Announce { info_hash, port })
                                .await;
                            let _ = self
                                .lpd_tx
                                .send(crate::lpd::LpdCommand::Announce { info_hash, port })
                                .await;
                            tracing::info!(%id, "Refreshed peer discovery via DHT and LPD");
                        }
                    }
                }
            }
            Command::GetFiles(id, reply_tx) => {
                let mut result = None;
                if let Some(task) = self.tasks.get(&id) {
                    if let Some(ext) = task
                        .extensions
                        .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
                    {
                        if let Ok(bt_task) = ext.clone().as_any_arc().downcast::<BtTask>() {
                            if let Some(torrent) = bt_task.state.torrent.lock().await.clone() {
                                if let Some(files) = torrent.info.files {
                                    result = Some(files);
                                } else if let Some(v2_files) = torrent.flatten_v2_files() {
                                    result = Some(
                                        v2_files
                                            .into_iter()
                                            .map(|f| crate::torrent::File {
                                                length: f.length,
                                                path: f.path,
                                                attr: None,
                                            })
                                            .collect(),
                                    );
                                } else if let Some(len) = torrent.info.length {
                                    // Single file torrent
                                    result = Some(vec![crate::torrent::File {
                                        length: len,
                                        path: vec![torrent.info.name.clone()],
                                        attr: None,
                                    }]);
                                }
                            }
                        }
                    }
                }
                let _ = reply_tx.send(result);
            }
            Command::SetFileSelection(id, selection) => {
                if let Some(task) = self.tasks.get_mut(&id) {
                    if let Some(ext) = task
                        .extensions
                        .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
                    {
                        if let Ok(bt_task) = ext.clone().as_any_arc().downcast::<BtTask>() {
                            task.selected_files = Some(selection.clone());
                            if let Some(torrent) = bt_task.state.torrent.lock().await.clone() {
                                let selected_pieces = torrent.compute_selected_pieces(&selection);
                                task.total_length = torrent.selected_total_length(&selection);
                                let mut picker_guard = bt_task.state.picker.lock().await;
                                if let Some(ref mut picker) = *picker_guard {
                                    let bt_config = self.config.load().bittorrent.clone();
                                    picker.selected_pieces = selected_pieces;
                                    picker.endgame_threshold_pieces =
                                        bt_config.endgame_threshold_pieces;
                                    picker.endgame_threshold_percent =
                                        bt_config.endgame_threshold_percent;
                                }
                            }
                            let _ = self.save_task(id).await;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
