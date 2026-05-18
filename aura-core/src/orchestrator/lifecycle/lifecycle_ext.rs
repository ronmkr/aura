use super::SubTaskEvent;
use crate::orchestrator::Orchestrator;
use crate::storage::StorageRequest;
use crate::task::TaskType;
use crate::worker::{ProtocolWorker, Segment};
use crate::{Error, Result, TaskId};
use tokio::sync::mpsc;
use tracing::debug;

impl Orchestrator {
    pub(crate) async fn dispatch_next_ranges(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
    ) -> Result<()> {
        let token = match self.cancellation_tokens.get(&meta_id) {
            Some(t) => t.clone(),
            None => return Ok(()),
        };

        if token.is_cancelled() {
            return Ok(());
        }

        let local_addr = self.resolve_local_addr();
        let config_arc = self.config.clone();
        loop {
            if token.is_cancelled() {
                break;
            }

            let meta_task = self
                .tasks
                .get_mut(&meta_id)
                .ok_or(Error::TaskNotFound(meta_id))?;

            let (uri, ttype, current_concurrency, target_concurrency) = {
                let sub_task = meta_task
                    .subtasks
                    .iter()
                    .find(|s| s.id == sub_id)
                    .ok_or_else(|| Error::Task(meta_id, "Subtask not found".to_string()))?;
                (
                    sub_task.uri.clone(),
                    sub_task.task_type.clone(),
                    sub_task.assigned_ranges.len(),
                    sub_task.target_concurrency,
                )
            };

            if current_concurrency >= target_concurrency {
                break;
            }

            if ttype == TaskType::BitTorrent {
                let worker_tx = self
                    .worker_command_txs
                    .get(&sub_id)
                    .cloned()
                    .unwrap_or_else(|| {
                        let (tx, _) = tokio::sync::broadcast::channel(1024);
                        tx
                    });
                let my_id = self.peer_id;
                let bt_task = match self.bt_tasks.get(&sub_id) {
                    Some(bt) => bt.clone(),
                    None => break,
                };

                let peer_opt = {
                    let mut registry = bt_task.state.registry.lock().await;
                    registry.get_peer_to_connect()
                };

                if let Some(peer) = peer_opt {
                    let peer_addr = format!("{}:{}", peer.ip, peer.port);
                    let info_hash = bt_task.state.info_hash;
                    let pool = self.pool.clone();
                    let proxy = config_arc.load().network.proxy.clone();
                    let throttler = self.throttler.clone();

                    let storage_tx = self.storage_tx.clone();
                    let subtask_tx = self.subtask_tx.clone();
                    let token_clone = token.clone();

                    let dummy_range = crate::task::Range { start: 0, end: 0 };
                    meta_task.in_flight_ranges.push((sub_id, dummy_range));
                    if let Some(sub) = meta_task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                        sub.assigned_ranges.push(dummy_range);
                    }

                    tracing::debug!(%meta_id, %sub_id, %peer_addr, "Spawning worker for peer");

                    tokio::spawn(async move {
                        let mut worker = crate::bt_worker::BtWorker::new(
                            peer_addr.clone(),
                            info_hash,
                            [0; 20],
                            my_id,
                            pool,
                            proxy,
                            throttler,
                        );
                        worker.local_addr = local_addr;

                        tokio::select! {
                            _ = token_clone.cancelled() => {}
                            res = worker.run_loop(
                                meta_id,
                                sub_id,
                                bt_task,
                                storage_tx,
                                subtask_tx.clone(),
                                worker_tx.subscribe(),
                                token_clone.clone(),
                            ) => {
                                if let Err(e) = res {
                                    tracing::debug!(%meta_id, %sub_id, error = %e, "BtWorker failed");
                                }
                                let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, dummy_range)).await;
                            }
                        }
                    });
                } else {
                    tracing::debug!(%meta_id, %sub_id, "peer_opt is None, breaking from dispatch_next_ranges");
                    break;
                }
            } else if let Some(range) = meta_task.pick_range_for_subtask(sub_id) {
                let storage_tx = self.storage_tx.clone();
                let subtask_tx = self.subtask_tx.clone();
                let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<u64>();
                let token_clone = token.clone();
                let config_clone = config_arc.clone();
                let throttler_clone = self.throttler.clone();

                let subtask_tx_progress = subtask_tx.clone();
                let progress_handle = tokio::spawn(async move {
                    while let Some(bytes) = progress_rx.recv().await {
                        let _ = subtask_tx_progress
                            .send(SubTaskEvent::Downloaded(meta_id, sub_id, bytes))
                            .await;
                    }
                });

                tokio::spawn(async move {
                    let config = config_clone.load();
                    match ttype {
                        TaskType::Http => {
                            let worker = crate::worker::WorkerBuilder::new(uri)
                                .local_addr(local_addr)
                                .user_agent(Some(config.network.user_agent.clone()))
                                .connect_timeout(Some(config.network.connect_timeout_secs))
                                .proxy(config.network.proxy.clone())
                                .retry_count(config.network.http_retry_count)
                                .retry_delay_secs(config.network.http_retry_delay_secs)
                                .build_http();
                            let segment = Segment {
                                offset: range.start,
                                length: range.length(),
                            };

                            tokio::select! {
                                _ = token_clone.cancelled() => {
                                }
                                res = worker.fetch_segment(meta_id, segment, Some(progress_tx), throttler_clone.clone()) => {
                                    // Ensure all progress events are forwarded before finishing the range
                                    let _ = progress_handle.await;

                                    match res {
                                        Ok(piece) => {
                                            let _ = storage_tx.send(StorageRequest::Write {
                                                task_id: meta_id,
                                                segment: piece.segment,
                                                data: piece.data,
                                            }).await;
                                            let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, range)).await;
                                        }
                                        Err(e) => {
                                            debug!(%meta_id, %sub_id, error = %e, "Range fetch failed");
                                            let _ = subtask_tx.send(crate::orchestrator::SubTaskEvent::Failed(meta_id, sub_id, e.to_string())).await;
                                        }
                                    }
                                }
                            }
                        }
                        TaskType::Ftp => {
                            let worker = crate::worker::WorkerBuilder::new(uri)
                                .local_addr(local_addr)
                                .build_ftp();
                            let segment = Segment {
                                offset: range.start,
                                length: range.length(),
                            };

                            tokio::select! {
                                _ = token_clone.cancelled() => {
                                }
                                res = worker.fetch_segment(meta_id, segment, Some(progress_tx), throttler_clone.clone()) => {
                                    match res {
                                        Ok(piece) => {
                                            let _ = storage_tx.send(StorageRequest::Write {
                                                task_id: meta_id,
                                                segment: piece.segment,
                                                data: piece.data,
                                            }).await;
                                            let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, range)).await;
                                        }
                                        Err(e) => {
                                            debug!(%meta_id, %sub_id, error = %e, "Range fetch failed");
                                            let _ = subtask_tx.send(crate::orchestrator::SubTaskEvent::Failed(meta_id, sub_id, e.to_string())).await;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                });
            } else {
                break;
            }
        }
        Ok(())
    }
}
