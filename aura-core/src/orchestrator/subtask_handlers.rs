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
            SubTaskEvent::RefreshMatured(meta_id, sub_id, metadata) => {
                self.handle_refresh_matured(meta_id, sub_id, metadata)
                    .await?;
            }
            SubTaskEvent::RefreshNotModified(meta_id, sub_id) => {
                self.handle_refresh_not_modified(meta_id, sub_id).await?;
            }
            SubTaskEvent::RefreshFailed(meta_id, sub_id, err) => {
                self.handle_refresh_failed(meta_id, sub_id, err).await?;
            }
            SubTaskEvent::MetadataReceived(meta_id, sub_id, torrent) => {
                self.handle_bt_metadata_received(meta_id, sub_id, *torrent)
                    .await?;
            }
            SubTaskEvent::RangeFinished(meta_id, sub_id, range) => {
                self.worker_cancellation_tokens.remove(&sub_id);
                self.handle_range_finished(meta_id, sub_id, range).await?;
            }
            SubTaskEvent::Failed(meta_id, sub_id, err) => {
                self.handle_subtask_failed(meta_id, sub_id, err).await?;
            }
            SubTaskEvent::Downloaded(meta_id, sub_id, bytes, peer_addr) => {
                if let Some(task) = self.tasks.get_mut(&meta_id) {
                    task.completed_length += bytes;
                    if let Some(sub) = task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                        sub.recent_bytes_downloaded += bytes;
                    }
                }

                if let Some(bt_task) = self.get_bt_task(sub_id) {
                    let mut registry = bt_task.state.registry.lock().await;
                    registry.add_downloaded(&peer_addr, bytes);
                }

                self.emit_progress(meta_id);
            }
            SubTaskEvent::Uploaded(meta_id, sub_id, bytes, peer_addr) => {
                if let Some(bt_task) = self.get_bt_task(sub_id) {
                    let mut registry = bt_task.state.registry.lock().await;
                    registry.add_uploaded(&peer_addr, bytes);
                }

                self.emit_progress(meta_id);
            }
            SubTaskEvent::PeerBitfield(meta_id, _peer_id, bf) => {
                debug!(%meta_id, count = bf.count_set(), "Peer bitfield received");
            }
            SubTaskEvent::PeerHave(meta_id, _peer_id, idx) => {
                debug!(%meta_id, idx, "Peer reported piece availability");
            }
            SubTaskEvent::PieceVerified(meta_id, sub_id, piece_idx) => {
                debug!(%meta_id, %sub_id, piece_idx, "Broadcasting cancellation for verified piece");
                if let Some(bt_task) = self.get_bt_task(sub_id) {
                    if let Some(tx) = self.worker_command_txs.get(&meta_id) {
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
                                if let Some(tx) = self.worker_command_txs.get(&meta_id) {
                                    let mut dropped_count = 0;
                                    for &piece_idx in &pending_pieces {
                                        if let Err(e) =
                                            tx.send(WorkerCommand::EndgameFetch(piece_idx))
                                        {
                                            dropped_count += 1;
                                            tracing::error!(%meta_id, %piece_idx, err = %e, "Failed to broadcast EndgameFetch; message dropped");
                                        }
                                    }
                                    if dropped_count > 0 {
                                        tracing::warn!(%meta_id, count = dropped_count, "Endgame: Some redundant requests were dropped due to full broadcast buffer");
                                    }
                                }
                            }
                        }
                    }
                }
            }
            SubTaskEvent::BtTaskRegistered(meta_id, info_hash, task, worker_cmd_tx) => {
                self.bt_registry.insert(info_hash, meta_id);
                if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
                    meta_task.extensions.insert(
                        crate::worker::bittorrent::BT_EXTENSION_KEY.to_string(),
                        task,
                    );
                }
                self.worker_command_txs.insert(meta_id, worker_cmd_tx);
            }
            SubTaskEvent::LpdPeerDiscovered(info_hash, peer) => {
                if let Some(meta_id) = self.bt_registry.get(&info_hash) {
                    if let Some(bt_task) = self.get_bt_task(*meta_id) {
                        let mut registry = bt_task.state.registry.lock().await;
                        registry.add_peers(vec![peer]);
                        let _ = self
                            .subtask_tx
                            .send(SubTaskEvent::PeersDiscovered(*meta_id))
                            .await;
                    }
                }
            }
            SubTaskEvent::PexPeersDiscovered(info_hash, peers) => {
                if let Some(meta_id) = self.bt_registry.get(&info_hash) {
                    if let Some(bt_task) = self.get_bt_task(*meta_id) {
                        let mut registry = bt_task.state.registry.lock().await;
                        registry.add_peers(peers);
                        let _ = self
                            .subtask_tx
                            .send(SubTaskEvent::PeersDiscovered(*meta_id))
                            .await;
                    }
                }
            }
            SubTaskEvent::KillSwitch => {
                let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                for id in ids {
                    let _ = self.handle_pause(id).await;
                }
            }
            SubTaskEvent::Retry(meta_id, sub_id) => {
                self.handle_retry_subtask(meta_id, sub_id).await?;
            }
            SubTaskEvent::ScrubberEvent(event) => {
                match event {
                    crate::scrubber::ScrubberEvent::PieceCorrupted(meta_id, piece_index) => {
                        tracing::warn!(%meta_id, piece_index, "Scrubber reported corrupted piece");
                        if let Some(task) = self.tasks.get(&meta_id) {
                            if let Some(bt_sub) = task
                                .subtasks
                                .iter()
                                .find(|s| s.task_type == crate::task::TaskType::BitTorrent)
                            {
                                if let Some(bt_task) = self.get_bt_task(bt_sub.id) {
                                    let mut bf_guard = bt_task.state.bitfield.lock().await;
                                    if let Some(ref mut bf) = *bf_guard {
                                        bf.set(piece_index, false); // Invalidate piece
                                    }
                                }
                                let _ = self
                                    .handle_command(crate::orchestrator::Command::RefreshDiscovery(
                                        meta_id,
                                    ))
                                    .await;
                            } else {
                                // For non-swarm, the whole file is corrupt. We can pause or mark as error.
                                let _ = self
                                    .handle_command(crate::orchestrator::Command::Pause(meta_id))
                                    .await;
                                let _ = self.event_tx.send(Event::TaskError {
                                    id: meta_id,
                                    message: "File corrupted".to_string(),
                                });
                            }
                        }
                    }
                    crate::scrubber::ScrubberEvent::ScrubComplete(meta_id) => {
                        tracing::info!(%meta_id, "Integrity scrub complete");
                    }
                    crate::scrubber::ScrubberEvent::ScrubFailed(meta_id, err) => {
                        tracing::error!(%meta_id, %err, "Integrity scrub failed");
                    }
                }
            }
            SubTaskEvent::RoamingDetected => {
                info!("Orchestrator handling network interface roaming event");
                let downloading_tasks: Vec<TaskId> = self
                    .tasks
                    .iter()
                    .filter(|(_, t)| {
                        t.phase == DownloadPhase::Downloading
                            || t.phase == DownloadPhase::MetadataExchange
                    })
                    .map(|(&id, _)| id)
                    .collect();

                if !downloading_tasks.is_empty() {
                    info!(
                        "Pausing {} active tasks to recycle connections for interface roaming",
                        downloading_tasks.len()
                    );
                    for id in &downloading_tasks {
                        let _ = self.handle_pause(*id).await;
                    }

                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                    info!(
                        "Resuming {} tasks on the new default route",
                        downloading_tasks.len()
                    );
                    for id in downloading_tasks {
                        if let Some(task) = self.tasks.get_mut(&id) {
                            task.phase = DownloadPhase::Downloading;
                            let _ = self.save_task(id).await;

                            let token = tokio_util::sync::CancellationToken::new();
                            self.cancellation_tokens.insert(id, token.clone());
                            let _ = self.start_task_loops_with_bitfield(id, token, None).await;
                        }
                    }
                }
            }
            SubTaskEvent::PeersDiscovered(meta_id) => {
                if let Some(task) = self.tasks.get(&meta_id) {
                    if let Some(bt_sub) = task
                        .subtasks
                        .iter()
                        .find(|s| s.task_type == crate::task::TaskType::BitTorrent)
                    {
                        let sub_id = bt_sub.id;
                        self.dispatch_next_ranges(meta_id, sub_id).await?;
                    }
                }
            }
        }
        Ok(())
    }
}
