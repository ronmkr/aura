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
                let bt_task = self.get_bt_task(sub_id);
                if let Some(task) = self.tasks.get_mut(&meta_id) {
                    if let Some(bt_task) = bt_task {
                        if let Some(tx) = self.worker_command_txs.get(&meta_id) {
                            let _ = tx.send(WorkerCommand::CancelPiece(piece_idx));
                        }

                        let mut bf_guard = bt_task.state.bitfield.lock().await;
                        if let Some(ref mut bf) = *bf_guard {
                            if !bf.get(piece_idx) {
                                bf.set(piece_idx, true);
                                if let Some(ref torrent) = *bt_task.state.torrent.lock().await {
                                    let piece_len = torrent.info.piece_length;
                                    let start = piece_idx as u64 * piece_len;
                                    let end = std::cmp::min(start + piece_len, task.total_length);
                                    task.completed_length += end.saturating_sub(start);
                                }
                            }
                        }

                        let mut picker_guard = bt_task.state.picker.lock().await;
                        if let Some(ref mut picker) = *picker_guard {
                            picker.mark_completed(piece_idx);
                        }
                    }
                }
            }
            SubTaskEvent::RecheckProgress(meta_id, progress) => {
                if let Some(task) = self.tasks.get_mut(&meta_id) {
                    task.recheck_progress = progress;
                }
                self.emit_progress(meta_id);
            }
            SubTaskEvent::RecheckComplete(meta_id, bitfield) => {
                let mut completed = false;
                let mut bt_sub_id = None;
                if let Some(task) = self.tasks.get(&meta_id) {
                    for sub in &task.subtasks {
                        if sub.task_type == crate::task::TaskType::BitTorrent {
                            bt_sub_id = Some(sub.id);
                            break;
                        }
                    }
                }

                let bt_task = if let Some(sub_id) = bt_sub_id {
                    self.get_bt_task(sub_id)
                } else {
                    None
                };

                if let Some(task) = self.tasks.get_mut(&meta_id) {
                    task.recheck_progress = 1.0;

                    if let Some(bt_task) = bt_task {
                        let mut bf_guard = bt_task.state.bitfield.lock().await;
                        *bf_guard = Some(bitfield.clone());
                        let mut picker_guard = bt_task.state.picker.lock().await;
                        if let Some(ref mut picker) = *picker_guard {
                            for i in 0..bitfield.len() {
                                if bitfield.get(i) {
                                    picker.mark_completed(i);
                                }
                            }
                        }
                        if let Some(ref torrent) = *bt_task.state.torrent.lock().await {
                            let piece_len = torrent.info.piece_length;
                            let mut len = 0;
                            for i in 0..bitfield.len() {
                                if bitfield.get(i) {
                                    let start = i as u64 * piece_len;
                                    let end = std::cmp::min(start + piece_len, task.total_length);
                                    len += end.saturating_sub(start);
                                }
                            }
                            task.completed_length = len;
                        }
                    } else {
                        let completed_pieces = bitfield.count_set();
                        let total_pieces = bitfield.len();
                        if total_pieces > 0 {
                            let piece_len = task.total_length.div_ceil(total_pieces as u64);
                            task.completed_length = std::cmp::min(
                                completed_pieces as u64 * piece_len,
                                task.total_length,
                            );
                        }

                        if bitfield.is_complete() {
                            task.completed_length = task.total_length;
                        } else {
                            task.generate_ranges(128, Some(&bitfield));
                        }
                    }

                    if task.completed_length >= task.total_length && task.total_length > 0 {
                        task.phase = DownloadPhase::Complete;
                        completed = true;
                    } else {
                        task.phase = DownloadPhase::Downloading;
                    }

                    let _ = self.save_task(meta_id).await;
                }

                if completed {
                    let _ = self.storage_client.complete(meta_id).await;
                }

                self.emit_progress(meta_id);

                let sub_ids: Vec<TaskId> = if let Some(t) = self.tasks.get(&meta_id) {
                    t.subtasks.iter().map(|s| s.id).collect()
                } else {
                    Vec::new()
                };

                for sub_id in sub_ids {
                    let _ = self.dispatch_next_ranges(meta_id, sub_id).await;
                }
            }
            SubTaskEvent::BtTaskRegistered(meta_id, info_hash, task, worker_cmd_tx) => {
                self.bt_registry.insert(info_hash, meta_id);
                let mut bt_sub_id = None;
                if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
                    if let Some(ratio) = meta_task.seed_ratio_override {
                        let mut seed_ratio_guard = task.state.seed_ratio.lock().unwrap();
                        *seed_ratio_guard = Some(ratio);
                    }
                    if let Some(time) = meta_task.seed_time_override {
                        let mut seed_time_guard = task.state.seed_time.lock().unwrap();
                        *seed_time_guard = Some(time);
                    }
                    meta_task.extensions.insert(
                        crate::worker::bittorrent::BT_EXTENSION_KEY.to_string(),
                        task.clone(),
                    );
                    bt_sub_id = meta_task
                        .subtasks
                        .iter()
                        .find(|s| s.task_type == crate::task::TaskType::BitTorrent)
                        .map(|s| s.id);
                }
                self.worker_command_txs.insert(meta_id, worker_cmd_tx);

                if let Some(sub_id) = bt_sub_id {
                    let _ = self.maybe_spawn_recheck(meta_id, sub_id).await;
                }
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

                    let delay_ms = self.config.load().network.roaming_reconnect_delay_ms;
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;

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
