use super::{Event, Orchestrator, SubTaskEvent, WorkerCommand};
use crate::task::DownloadPhase;
use crate::{Result, TaskId};
use tracing::{debug, info};

impl Orchestrator {
    pub(crate) async fn handle_subtask_event(&mut self, event: SubTaskEvent) -> Result<()> {
        match event {
            SubTaskEvent::Matured(meta_id, sub_id, metadata) => {
                self.handle_subtask_matured(meta_id, sub_id, metadata)
                    .await?;
            }
            SubTaskEvent::MetadataReceived(meta_id, sub_id, torrent) => {
                self.handle_bt_metadata_received(meta_id, sub_id, *torrent)
                    .await?;
            }
            SubTaskEvent::RangeFinished(meta_id, sub_id, range) => {
                self.handle_range_finished(meta_id, sub_id, range).await?;
            }
            SubTaskEvent::Failed(meta_id, sub_id, err) => {
                info!(%meta_id, %sub_id, %err, "Subtask failed");
            }
            SubTaskEvent::Downloaded(meta_id, sub_id, bytes) => {
                if let Some(task) = self.tasks.get_mut(&meta_id) {
                    task.completed_length += bytes;
                    if let Some(sub) = task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                        sub.recent_bytes_downloaded += bytes;
                    }
                    let _ = self.event_tx.send(Event::TaskProgress {
                        id: meta_id,
                        completed_bytes: task.completed_length,
                        uploaded_bytes: task.uploaded_length,
                        total_bytes: task.total_length,
                    });
                }
            }
            SubTaskEvent::Uploaded(meta_id, bytes) => {
                if let Some(task) = self.tasks.get_mut(&meta_id) {
                    task.uploaded_length += bytes;
                    let _ = self.event_tx.send(Event::TaskProgress {
                        id: meta_id,
                        completed_bytes: task.completed_length,
                        uploaded_bytes: task.uploaded_length,
                        total_bytes: task.total_length,
                    });
                }
            }
            SubTaskEvent::PeerBitfield(meta_id, peer_id, bf) => {
                debug!(%meta_id, ?peer_id, count = bf.count_set(), "Peer bitfield received");
            }
            SubTaskEvent::PeerHave(meta_id, peer_id, idx) => {
                debug!(%meta_id, ?peer_id, idx, "Peer reported piece availability");
            }
            SubTaskEvent::PieceVerified(meta_id, sub_id, piece_idx) => {
                debug!(%meta_id, %sub_id, piece_idx, "Broadcasting cancellation for verified piece");
                if let Some(bt_task) = self.bt_tasks.get(&sub_id) {
                    if let Some(tx) = self.worker_command_txs.get(&sub_id) {
                        let _ = tx.send(WorkerCommand::CancelPiece(piece_idx));
                    }

                    // Endgame coordination: if we are in endgame, we might want to assign
                    // this newly available worker capacity to other pending pieces.
                    let bf_guard = bt_task.state.bitfield.lock().await;
                    let picker_guard = bt_task.state.picker.lock().await;
                    if let (Some(bf), Some(picker)) = (bf_guard.as_ref(), picker_guard.as_ref()) {
                        if picker.is_endgame(bf) {
                            let mut pending_pieces = Vec::new();
                            for i in 0..bf.len() {
                                if !bf.get(i) {
                                    pending_pieces.push(i);
                                }
                            }

                            if !pending_pieces.is_empty() {
                                debug!(%meta_id, pending = %pending_pieces.len(), "Endgame: broadcasting redundant requests");
                                if let Some(tx) = self.worker_command_txs.get(&sub_id) {
                                    for &piece_idx in &pending_pieces {
                                        let _ = tx.send(WorkerCommand::RequestPiece(piece_idx));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            SubTaskEvent::BtTaskRegistered(sub_id, info_hash, task, worker_cmd_tx) => {
                self.bt_registry.insert(info_hash, task.clone());
                self.bt_tasks.insert(sub_id, task);
                self.worker_command_txs.insert(sub_id, worker_cmd_tx);
            }
            SubTaskEvent::LpdPeerDiscovered(info_hash, peer) => {
                if let Some(bt_task) = self.bt_registry.get(&info_hash) {
                    let mut registry = bt_task.state.registry.lock().await;
                    registry.add_peers(vec![peer]);
                }
            }
            SubTaskEvent::KillSwitch => {
                let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                for id in ids {
                    let _ = self.handle_pause(id).await;
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn handle_bt_metadata_received(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        torrent: crate::torrent::Torrent,
    ) -> Result<()> {
        if let Some(bt_task) = self.bt_tasks.get(&sub_id) {
            bt_task.state.mature(torrent.clone()).await;

            let metadata = crate::worker::Metadata {
                final_uri: format!("magnet:?xt={}", bt_task.state.info_hash.to_magnet_urn()),
                total_length: Some(torrent.total_length()),
                name: Some(torrent.info.name.clone()),
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
            // Update name if currently unnamed
            if (meta_task.name == "unnamed" || meta_task.name.is_empty()) && metadata.name.is_some()
            {
                let new_name = metadata.name.clone().unwrap();
                info!(%meta_id, %new_name, "Updating task name from metadata");
                meta_task.name = new_name;

                // Update storage engine
                let path = std::env::current_dir()
                    .unwrap_or_default()
                    .join(&meta_task.name);
                let _ = self
                    .storage_tx
                    .send(crate::storage::StorageRequest::RegisterTask {
                        task_id: meta_id,
                        path,
                        total_length: meta_task.total_length,
                    })
                    .await;
            }

            if meta_task.total_length == 0 {
                if let Some(len) = metadata.total_length {
                    info!(%meta_id, %len, "Metadata matured: task initialized");
                    meta_task.total_length = len;
                    meta_task.generate_ranges(16); // Default 16 segments
                }
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
                }
            }

            meta_task.mark_range_complete(sub_id, range);

            if meta_task.is_complete() {
                info!(%meta_id, "All ranges complete for MetaTask, entering seeding phase");
                meta_task.phase = DownloadPhase::Complete;
                if meta_task.seeding_start_time.is_none() {
                    meta_task.seeding_start_time = Some(chrono::Utc::now());
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

    pub(crate) async fn handle_storage_completion(&mut self, id: TaskId) -> Result<()> {
        info!(%id, "Storage reported completion");
        if let Some(task) = self.tasks.get(&id) {
            let _ = self.event_tx.send(Event::TaskProgress {
                id,
                completed_bytes: task.total_length,
                uploaded_bytes: task.uploaded_length,
                total_bytes: task.total_length,
            });
        }
        let _ = self.event_tx.send(Event::TaskCompleted(id));
        Ok(())
    }
}
