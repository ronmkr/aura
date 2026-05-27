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
                if let Some(task) = self.tasks.get_mut(&meta_id) {
                    if let Some(sub) = task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                        sub.active = false;
                        let is_fatal =
                            err.contains("404") || err.contains("403") || err.contains("401");
                        sub.retry_count += 1;

                        let config = self.config.load();
                        let max_retries = config.network.http_retry_count;
                        let delay_base = config.network.http_retry_delay_secs;

                        if sub.retry_count < max_retries && !is_fatal {
                            sub.phase = DownloadPhase::Degraded;
                            tracing::warn!(%meta_id, %sub_id, count = sub.retry_count, "Mirror degraded, recycling ranges");

                            // Self-healing: Schedule retry with exponential backoff
                            let subtask_tx = self.subtask_tx.clone();
                            let retry_delay =
                                std::time::Duration::from_secs(sub.retry_count as u64 * delay_base);
                            tokio::spawn(async move {
                                tokio::time::sleep(retry_delay).await;
                                let _ = subtask_tx.send(SubTaskEvent::Retry(meta_id, sub_id)).await;
                            });
                        } else {
                            sub.phase = DownloadPhase::Error;
                            if is_fatal {
                                tracing::error!(%meta_id, %sub_id, "Mirror permanently failed due to fatal error: {}", err);
                            } else {
                                tracing::error!(%meta_id, %sub_id, "Mirror permanently failed after {} retries", max_retries);
                            }
                            task.blacklisted_uris.push(sub.uri.clone());
                        }

                        // Failover: Return assigned ranges to the pending pool
                        let failed_ranges = std::mem::take(&mut sub.assigned_ranges);
                        for r in failed_ranges {
                            task.pending_ranges.push(r);
                            task.in_flight_ranges
                                .retain(|(sid, rng)| *sid != sub_id || *rng != r);
                        }
                    }

                    if task
                        .subtasks
                        .iter()
                        .all(|s| s.phase == DownloadPhase::Error)
                    {
                        task.phase = DownloadPhase::Error;
                        let event = Event::TaskError {
                            id: meta_id,
                            message: err,
                        };
                        let _ = self.event_tx.send(event.clone());
                        self.hook_manager.handle_event(&event).await;
                    } else {
                        // Trigger next range dispatch for other active subtasks
                        let active_subs: Vec<TaskId> = task
                            .subtasks
                            .iter()
                            .filter(|s| s.active)
                            .map(|s| s.id)
                            .collect();
                        for aid in active_subs {
                            let _ = self.dispatch_next_ranges(meta_id, aid).await;
                        }
                    }
                }
            }
            SubTaskEvent::Downloaded(meta_id, sub_id, bytes, peer_addr) => {
                if let Some(task) = self.tasks.get_mut(&meta_id) {
                    task.completed_length += bytes;
                    if let Some(sub) = task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                        sub.recent_bytes_downloaded += bytes;
                    }
                    if let Some(bt_task) = self.bt_tasks.get(&sub_id) {
                        let mut registry = bt_task.state.registry.lock().await;
                        registry.add_downloaded(&peer_addr, bytes);
                    }
                    let _ = self.event_tx.send(Event::TaskProgress {
                        id: meta_id,
                        completed_bytes: task.completed_length,
                        uploaded_bytes: task.uploaded_length,
                        total_bytes: task.total_length,
                    });
                }
            }
            SubTaskEvent::Uploaded(meta_id, sub_id, bytes, peer_addr) => {
                if let Some(task) = self.tasks.get_mut(&meta_id) {
                    task.uploaded_length += bytes;
                    if let Some(bt_task) = self.bt_tasks.get(&sub_id) {
                        let mut registry = bt_task.state.registry.lock().await;
                        registry.add_uploaded(&peer_addr, bytes);
                    }
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
                    let bf_guard: tokio::sync::MutexGuard<Option<crate::bitfield::Bitfield>> =
                        bt_task.state.bitfield.lock().await;
                    let picker_guard: tokio::sync::MutexGuard<
                        Option<crate::piece_picker::PiecePicker>,
                    > = bt_task.state.picker.lock().await;
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
                self.bt_registry.insert(info_hash, sub_id);
                self.bt_tasks.insert(sub_id, task);
                self.worker_command_txs.insert(sub_id, worker_cmd_tx);
            }
            SubTaskEvent::LpdPeerDiscovered(info_hash, peer) => {
                if let Some(meta_id) = self.bt_registry.get(&info_hash) {
                    if let Some(bt_task) = self.bt_tasks.get(meta_id) {
                        let mut registry = bt_task.state.registry.lock().await;
                        registry.add_peers(vec![peer]);
                    }
                }
            }
            SubTaskEvent::PexPeersDiscovered(info_hash, peers) => {
                if let Some(meta_id) = self.bt_registry.get(&info_hash) {
                    if let Some(bt_task) = self.bt_tasks.get(meta_id) {
                        let mut registry = bt_task.state.registry.lock().await;
                        registry.add_peers(peers);
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
                                if let Some(bt_task) = self.bt_tasks.get(&bt_sub.id) {
                                    let mut bf_guard = bt_task.state.bitfield.lock().await;
                                    if let Some(bf) = bf_guard.as_mut() {
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
        }
        Ok(())
    }
}
