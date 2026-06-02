use crate::orchestrator::Orchestrator;
use crate::task::DownloadPhase;
use crate::worker::bittorrent::task::BtTask;
use crate::{Result, TaskId};
use tracing::{debug, info};

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
                padding_ranges: torrent.get_padding_ranges(),
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
                        let bt_bitfield = meta_task
                            .extensions
                            .get("bittorrent")
                            .and_then(|e| e.clone().as_any_arc().downcast::<BtTask>().ok())
                            .and(None); // Simplified as we can't easily block for bitfield here

                        meta_task.generate_ranges(128, bt_bitfield);
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
                    let base_dir: std::path::PathBuf = if let Some(ref tid) = meta_task.tenant_id {
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
                        padding_ranges: metadata.padding_ranges.clone(),
                    })
                    .await;
            }

            if let Some(sub_task) = meta_task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                sub_task.phase = DownloadPhase::Downloading;
                if let Some(len) = metadata.total_length {
                    sub_task.total_length = len;
                }
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
            let _ = self
                .event_tx
                .send(crate::orchestrator::Event::MetadataResolved {
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
            // CRITICAL: Skip racing logic for BitTorrent dummy ranges (start=0, end=0)
            if range.start != 0 || range.end != 0 {
                let racing_sub_ids: Vec<TaskId> = meta_task
                    .in_flight_ranges
                    .iter()
                    .filter(|(sid, r)| *r == range && *sid != sub_id)
                    .map(|(sid, _)| *sid)
                    .collect();

                if !racing_sub_ids.is_empty() {
                    debug!(%meta_id, ?range, racing = racing_sub_ids.len(), "Range finished; canceling racing workers");
                    for racing_sid in racing_sub_ids {
                        if let Some(sub) =
                            meta_task.subtasks.iter_mut().find(|s| s.id == racing_sid)
                        {
                            sub.assigned_ranges.retain(|r| *r != range);
                        }
                        if let Some(w_token) = self.worker_cancellation_tokens.remove(&racing_sid) {
                            w_token.cancel();
                        }
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

        // Notify idle workers to check for more work
        if let Some(tx) = self.worker_command_txs.get(&meta_id) {
            let _ = tx.send(crate::orchestrator::WorkerCommand::CheckWork);
        }

        self.dispatch_next_ranges(meta_id, sub_id).await
    }
}
