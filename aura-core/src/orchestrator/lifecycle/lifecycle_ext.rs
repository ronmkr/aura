use crate::orchestrator::Orchestrator;
use crate::task::DownloadPhase;
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

            let selected_files = self
                .tasks
                .get(&meta_id)
                .and_then(|t| t.selected_files.clone());
            let total_length = if let Some(ref selection) = selected_files {
                torrent.selected_total_length(selection)
            } else {
                torrent.total_length()
            };

            let metadata = crate::worker::Metadata {
                final_uri: format!("magnet:?xt={}", bt_task.state.info_hash.to_magnet_urn()),
                total_length: Some(total_length),
                name: Some(torrent.info.name.clone()),
                range_supported: true,
                padding_ranges: torrent.get_padding_ranges(selected_files.as_deref()),
                etag: None,
                last_modified: None,
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
        let mut needs_reregister = false;
        let mut matured_metadata = None;

        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
            if meta_task.total_length == 0 {
                meta_task.range_supported = metadata.range_supported;
                if let Some(len) = metadata.total_length {
                    info!(%meta_id, %len, "Metadata matured: task initialized");
                    meta_task.total_length = len;
                    if metadata.range_supported {
                        meta_task.generate_ranges(128, None);
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
                    meta_task.range_supported = false;
                    meta_task.pending_ranges.push(crate::task::Range {
                        start: 0,
                        end: u64::MAX,
                    });
                    needs_reregister = true;
                }
            }

            if metadata.etag.is_some() {
                meta_task.etag = metadata.etag.clone();
                needs_reregister = true;
            }
            if metadata.last_modified.is_some() {
                meta_task.last_modified = metadata.last_modified.clone();
                needs_reregister = true;
            }

            if let Some(new_name) = metadata.name.clone() {
                if meta_task.name == crate::DEFAULT_TASK_NAME || meta_task.name.is_empty() {
                    meta_task.name = new_name;
                    needs_reregister = true;
                }
            }

            matured_metadata = Some((
                meta_task.tenant_id.clone(),
                meta_task.name.clone(),
                meta_task.total_length,
                meta_task.checksum.clone(),
            ));
        }

        if needs_reregister {
            if let Some((tenant_id, _, total_length, checksum)) = matured_metadata.as_ref() {
                let base_dir = self.resolve_base_dir(tenant_id);
                // We need meta_task for resolve_path, but we can't borrow it again mutably while matured_metadata exists if we were using references.
                // But matured_metadata is owned.
                let path = if let Some(meta_task) = self.tasks.get(&meta_id) {
                    self.mapping_engine.resolve_path(meta_task, &base_dir)
                } else {
                    base_dir.join(crate::DEFAULT_TASK_NAME)
                };

                let _ = self
                    .storage_tx
                    .send(crate::storage::StorageRequest::RegisterTask {
                        task_id: meta_id,
                        path,
                        total_length: *total_length,
                        checksum: checksum.clone(),
                        padding_ranges: metadata.padding_ranges.clone(),
                    })
                    .await;
            }
        }

        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
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

        let recheck_spawned = self.maybe_spawn_recheck(meta_id, sub_id).await?;
        let is_bt = if let Some(t) = self.tasks.get(&meta_id) {
            t.subtasks
                .iter()
                .any(|s| s.id == sub_id && s.task_type == crate::task::TaskType::BitTorrent)
        } else {
            false
        };

        if !recheck_spawned || is_bt {
            self.dispatch_next_ranges(meta_id, sub_id).await?;
        }
        Ok(())
    }

    pub(crate) async fn handle_range_finished(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        range: crate::task::Range,
    ) -> Result<()> {
        let mut completed = false;
        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
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

            let mut is_bt_complete = false;
            if let Some(bt_task) = meta_task
                .extensions
                .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
                .and_then(|e| {
                    e.clone()
                        .as_any_arc()
                        .downcast::<crate::worker::bittorrent::task::BtTask>()
                        .ok()
                })
            {
                let bf_guard = bt_task.state.bitfield.lock().await;
                let picker_guard = bt_task.state.picker.lock().await;
                if let (Some(b), Some(picker)) = (bf_guard.as_ref(), picker_guard.as_ref()) {
                    let mut complete = true;
                    for i in 0..picker.num_pieces {
                        if picker.selected_pieces.get(i) && !b.get(i) {
                            complete = false;
                            break;
                        }
                    }
                    is_bt_complete = complete;
                }
            }

            if (meta_task.is_complete() || is_bt_complete)
                && meta_task.phase != DownloadPhase::Verifying
                && meta_task.phase != DownloadPhase::Complete
            {
                if meta_task.checksum.is_some() {
                    info!(%meta_id, "All ranges complete for MetaTask, entering Verifying phase");
                    meta_task.phase = DownloadPhase::Verifying;
                } else {
                    info!(%meta_id, "All ranges complete for MetaTask, entering seeding phase");
                    meta_task.phase = DownloadPhase::Complete;
                    if let Some(bt) = self.get_bt_task(meta_id) {
                        let mut start_time = bt.state.seeding_start_time.lock().unwrap();
                        if start_time.is_none() {
                            *start_time = Some(chrono::Utc::now());
                        }
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

        if let Some(tx) = self.worker_command_txs.get(&meta_id) {
            let _ = tx.send(crate::orchestrator::WorkerCommand::CheckWork);
        }

        self.dispatch_next_ranges(meta_id, sub_id).await
    }
}
