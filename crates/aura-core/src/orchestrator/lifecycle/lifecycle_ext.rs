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
        let concurrency_per_subtask = 4;

        loop {
            if token.is_cancelled() {
                break;
            }

            let meta_task = self
                .tasks
                .get_mut(&meta_id)
                .ok_or_else(|| Error::Config("Task not found".to_string()))?;

            let (uri, ttype, current_concurrency) = {
                let sub_task = meta_task
                    .subtasks
                    .iter()
                    .find(|s| s.id == sub_id)
                    .ok_or_else(|| Error::Config("Subtask not found".to_string()))?;
                (
                    sub_task.uri.clone(),
                    sub_task.task_type.clone(),
                    sub_task.assigned_ranges.len(),
                )
            };

            if current_concurrency >= concurrency_per_subtask {
                break;
            }

            if let Some(range) = meta_task.pick_range_for_subtask(sub_id) {
                let storage_tx = self.storage_tx.clone();
                let subtask_tx = self.subtask_tx.clone();
                let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<u64>();
                let token_clone = token.clone();
                let config_clone = config_arc.clone();

                let subtask_tx_progress = subtask_tx.clone();
                tokio::spawn(async move {
                    while let Some(bytes) = progress_rx.recv().await {
                        let _ = subtask_tx_progress
                            .send(SubTaskEvent::Downloaded(meta_id, bytes))
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
                                .build_http();
                            let segment = Segment {
                                offset: range.start,
                                length: range.length(),
                            };

                            tokio::select! {
                                _ = token_clone.cancelled() => {
                                }
                                res = worker.fetch_segment(meta_id, segment, Some(progress_tx)) => {
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
                                            let _ = subtask_tx.send(SubTaskEvent::Failed(meta_id, sub_id, e.to_string())).await;
                                        }
                                    }
                                }
                            }
                        }
                        TaskType::BitTorrent => {}
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
                                res = worker.fetch_segment(meta_id, segment, Some(progress_tx)) => {
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
                                            let _ = subtask_tx.send(SubTaskEvent::Failed(meta_id, sub_id, e.to_string())).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            } else {
                break;
            }
        }
        Ok(())
    }
}
