use super::{Event, Orchestrator};
use crate::task::DownloadPhase;
use crate::{Result, TaskId};
use tracing::{debug, error, info};

impl Orchestrator {
    pub(crate) async fn handle_bt_metadata_received(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        torrent: crate::torrent::Torrent,
    ) -> Result<()> {
        if let Some(bt_task) = self.get_bt_task(sub_id) {
            bt_task.state.mature(torrent.clone()).await;

            let metadata = crate::worker::Metadata {
                final_uri: format!("magnet:?xt={}", bt_task.state.info_hash.to_magnet_urn()),
                total_length: Some(torrent.total_length()),
                name: Some(torrent.info.name.clone()),
                range_supported: true,
            };
            self.handle_subtask_matured(meta_id, sub_id, metadata)
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn handle_subtask_matured(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        metadata: crate::worker::Metadata,
    ) -> Result<()> {
        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
            let mut needs_reregister = false;

            if meta_task.total_length == 0 {
                meta_task.range_supported = metadata.range_supported;
                if let Some(len) = metadata.total_length {
                    info!(%meta_id, %len, "Metadata matured: task initialized");
                    meta_task.total_length = len;
                    if metadata.range_supported {
                        meta_task.generate_ranges(128); // Default 128 segments to allow high concurrency
                    } else {
                        info!(%meta_id, "Server does not support Range requests. Falling back to single-stream download.");
                        meta_task
                            .pending_ranges
                            .push(crate::task::Range { start: 0, end: len });
                    }
                    needs_reregister = true;
                } else {
                    info!(%meta_id, "Metadata matured but total length is unknown. Falling back to single-stream download.");
                    meta_task.total_length = 0;
                    meta_task.range_supported = false; // Unknown length implies single stream for now
                    meta_task.pending_ranges.push(crate::task::Range {
                        start: 0,
                        end: u64::MAX,
                    });
                    needs_reregister = true;
                }
            }

            // Update name if currently unnamed or if server provides a better one (with extension)
            if let Some(new_name) = metadata.name.clone() {
                let current_path = std::path::Path::new(&meta_task.name);
                let new_path = std::path::Path::new(&new_name);

                let current_has_ext = current_path.extension().is_some();
                let new_has_ext = new_path.extension().is_some();

                // Logic:
                // 1. If currently "unnamed" or empty, always accept.
                // 2. If current has no extension but new one does, accept.
                // 3. If new name is different and has a known mime type that isn't generic octet-stream, accept.
                let is_better_name = {
                    let new_guess = mime_guess::from_path(new_path).first();
                    let current_guess = mime_guess::from_path(current_path).first();

                    new_has_ext
                        && (!current_has_ext || (new_guess.is_some() && new_guess != current_guess))
                };

                if meta_task.name == "unnamed" || meta_task.name.is_empty() || is_better_name {
                    info!(%meta_id, %new_name, "Updating task name from metadata");
                    meta_task.name = new_name;
                    needs_reregister = true;
                }
            }

            if needs_reregister {
                // Update storage engine
                let path = {
                    let config = self.config.load();
                    let base_dir = if let Some(ref tid) = meta_task.tenant_id {
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
                    self.mapping_engine.resolve_path(meta_task, &base_dir)
                };
                let _ = self
                    .storage_tx
                    .send(crate::storage::StorageRequest::RegisterTask {
                        task_id: meta_id,
                        path,
                        total_length: meta_task.total_length,
                        checksum: meta_task.checksum.clone(),
                    })
                    .await;
            }

            if let Some(sub_task) = meta_task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                sub_task.phase = DownloadPhase::Downloading;
            }
        }

        let (should_notify, final_uri, total_length, name) =
            if let Some(meta_task) = self.tasks.get(&meta_id) {
                (
                    meta_task.total_length > 0,
                    metadata.final_uri,
                    meta_task.total_length,
                    metadata.name,
                )
            } else {
                (false, String::new(), 0, None)
            };

        if should_notify {
            let _ = self.event_tx.send(Event::MetadataResolved {
                id: meta_id,
                final_uri,
                total_length,
                name,
            });
        }

        self.dispatch_next_ranges(meta_id, sub_id).await
    }

    pub(crate) async fn handle_range_finished(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        range: crate::task::Range,
    ) -> Result<()> {
        let mut completed = false;
        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
            // Racing coordination: check if other subtasks were also working on this range
            let racing_sub_ids: Vec<TaskId> = meta_task
                .in_flight_ranges
                .iter()
                .filter(|(sid, r)| *r == range && *sid != sub_id)
                .map(|(sid, _)| *sid)
                .collect();

            if !racing_sub_ids.is_empty() {
                debug!(%meta_id, ?range, racing = racing_sub_ids.len(), "Range finished; canceling racing workers");
                for racing_sid in racing_sub_ids {
                    // For non-BT tasks, we rely on the in_flight_ranges cleanup and next loop check.
                    if let Some(sub) = meta_task.subtasks.iter_mut().find(|s| s.id == racing_sid) {
                        sub.assigned_ranges.retain(|r| *r != range);
                    }
                    if let Some(w_token) = self.worker_cancellation_tokens.remove(&racing_sid) {
                        w_token.cancel();
                    }
                }
            }

            meta_task.mark_range_complete(sub_id, range);

            if meta_task.is_complete()
                && meta_task.phase != DownloadPhase::Verifying
                && meta_task.phase != DownloadPhase::Complete
            {
                if meta_task.checksum.is_some() {
                    info!(%meta_id, "All ranges complete for MetaTask, entering Verifying phase");
                    meta_task.phase = DownloadPhase::Verifying;
                } else {
                    info!(%meta_id, "All ranges complete for MetaTask, entering seeding phase");
                    meta_task.phase = DownloadPhase::Complete;
                    if meta_task.seeding_start_time.is_none() {
                        meta_task.seeding_start_time = Some(chrono::Utc::now());
                    }
                }
                completed = true;
            }
        }

        if completed {
            let _ = self
                .storage_tx
                .send(crate::storage::StorageRequest::Complete(meta_id))
                .await;
        }

        self.dispatch_next_ranges(meta_id, sub_id).await
    }

    pub(crate) async fn handle_storage_event(
        &mut self,
        event: crate::storage::StorageEvent,
    ) -> Result<()> {
        match event {
            crate::storage::StorageEvent::Completed(id) => {
                info!(%id, "Storage reported completion");
                if let Some(task) = self.tasks.get_mut(&id) {
                    task.phase = DownloadPhase::Complete;
                    if task.seeding_start_time.is_none() {
                        task.seeding_start_time = Some(chrono::Utc::now());
                    }
                    let _ = self.event_tx.send(Event::TaskProgress {
                        id,
                        completed_bytes: task.total_length,
                        uploaded_bytes: task.uploaded_length,
                        total_bytes: task.total_length,
                    });

                    let (follow_on, tenant_id, priority, streaming_mode, task_name) =
                        if let Some(task) = self.tasks.get(&id) {
                            (
                                task.follow_on.clone(),
                                task.tenant_id.clone(),
                                task.priority,
                                task.streaming_mode,
                                task.name.clone(),
                            )
                        } else {
                            (None, None, 0, false, String::new())
                        };

                    // Handle follow-on actions (ADR 0029)
                    if let Some(follow_on) = follow_on {
                        let config = self.config.load();
                        let base_dir = if let Some(ref tid) = tenant_id {
                            if let Some(ctx) = self.tenants.get(tid) {
                                ctx.disk_path_root.clone().unwrap_or_else(|| {
                                    std::path::PathBuf::from(&config.storage.download_dir)
                                })
                            } else {
                                std::path::PathBuf::from(&config.storage.download_dir)
                            }
                        } else {
                            std::path::PathBuf::from(&config.storage.download_dir)
                        };
                        let file_path = base_dir.join(&task_name);

                        match follow_on {
                            crate::task::FollowOnAction::AutoStartTorrent => {
                                if file_path
                                    .extension()
                                    .map(|e| e == "torrent")
                                    .unwrap_or(false)
                                {
                                    info!(%id, ?file_path, "Auto-starting follow-on Torrent task");
                                    let new_id = TaskId(rand::random());
                                    let _ = self
                                        .handle_add_task(
                                            new_id,
                                            tenant_id.clone(),
                                            "unnamed".to_string(),
                                            vec![(
                                                file_path.to_string_lossy().to_string(),
                                                crate::task::TaskType::BitTorrent,
                                            )],
                                            None,
                                            priority,
                                            streaming_mode,
                                            Vec::new(),
                                            None,
                                        )
                                        .await;
                                }
                            }
                            crate::task::FollowOnAction::AutoStartMetalink => {
                                if file_path
                                    .extension()
                                    .map(|e| e == "metalink" || e == "meta4")
                                    .unwrap_or(false)
                                {
                                    info!(%id, ?file_path, "Auto-starting follow-on Metalink task");
                                    let new_id = TaskId(rand::random());
                                    let _ = self
                                        .handle_add_task(
                                            new_id,
                                            tenant_id.clone(),
                                            "unnamed".to_string(),
                                            vec![(
                                                file_path.to_string_lossy().to_string(),
                                                crate::task::TaskType::Http,
                                            )], // Task type doesn't strictly matter for metalink entry point
                                            None,
                                            priority,
                                            streaming_mode,
                                            Vec::new(),
                                            None,
                                        )
                                        .await;
                                }
                            }
                            crate::task::FollowOnAction::Custom(uri) => {
                                info!(%id, %uri, "Starting custom follow-on task");
                                let new_id = TaskId(rand::random());
                                let _ = self
                                    .handle_add_task(
                                        new_id,
                                        tenant_id.clone(),
                                        "unnamed".to_string(),
                                        vec![(uri, crate::task::TaskType::Http)],
                                        None,
                                        priority,
                                        streaming_mode,
                                        Vec::new(),
                                        None,
                                    )
                                    .await;
                            }
                        }
                    }
                }
                let _ = self.event_tx.send(Event::TaskCompleted(id));
                self.check_waiting_tasks().await;
            }
            crate::storage::StorageEvent::Error(id, err) => {
                error!(%id, %err, "Storage reported fatal error; pausing task");
                let mut exists = false;
                if let Some(task) = self.tasks.get_mut(&id) {
                    task.phase = DownloadPhase::Error;
                    exists = true;
                }

                if exists {
                    // Trigger pause logic to cleanup workers
                    let _ = self.handle_pause(id).await;

                    let _ = self.event_tx.send(Event::TaskError {
                        id,
                        message: format!("Storage Error: {}", err),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "event_handlers_tests.rs"]
mod tests;
