use super::SubTaskEvent;
use crate::orchestrator::Orchestrator;
use crate::task::{DownloadPhase, TaskType};
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
        tracing::debug!(%meta_id, %sub_id, "Dispatching next ranges");
        let token = match self.cancellation_tokens.get(&meta_id) {
            Some(t) => t.clone(),
            None => return Ok(()),
        };

        if token.is_cancelled() {
            return Ok(());
        }

        loop {
            if token.is_cancelled() {
                break;
            }

            let (uri, ttype, current_concurrency, target_concurrency, is_error, tenant_id_clone) = {
                let meta_task = self
                    .tasks
                    .get(&meta_id)
                    .ok_or(Error::TaskNotFound(meta_id))?;

                let is_error = meta_task.phase == DownloadPhase::Error;
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
                    is_error,
                    meta_task.tenant_id.clone(),
                )
            };

            if is_error {
                break;
            }

            if current_concurrency >= target_concurrency {
                break;
            }

            if ttype == TaskType::BitTorrent {
                match self.dispatch_bittorrent_peer(meta_id, sub_id, &token).await {
                    Ok(true) => {}
                    _ => break,
                }
            } else {
                let range = {
                    let meta_task = self
                        .tasks
                        .get_mut(&meta_id)
                        .ok_or(Error::TaskNotFound(meta_id))?;
                    meta_task.pick_range_for_subtask(sub_id)
                };

                if let Some(range) = range {
                    let storage_client = self.storage_client.clone();
                    let subtask_tx = self.subtask_tx.clone();
                    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel::<u64>();
                    let child_token = token.child_token();
                    self.worker_cancellation_tokens
                        .insert(sub_id, child_token.clone());
                    let token_clone = child_token;
                    let throttler_clone = self.throttler.clone();
                    let orchestrator_handle = self.handle();

                    let subtask_tx_progress = subtask_tx.clone();
                    let progress_handle = tokio::spawn(async move {
                        while let Some(bytes) = progress_rx.recv().await {
                            let _ = subtask_tx_progress
                                .send(SubTaskEvent::Downloaded(
                                    meta_id,
                                    sub_id,
                                    bytes,
                                    String::new(),
                                ))
                                .await;
                        }
                    });

                    tokio::spawn(async move {
                        match ttype {
                            TaskType::Http => {
                                tracing::debug!(%meta_id, %sub_id, ?range, "Spawning HTTP worker for range");
                                let worker = orchestrator_handle
                                    .build_worker_builder(uri, tenant_id_clone)
                                    .build_http();
                                let segment = Segment {
                                    offset: range.start,
                                    length: range.length(),
                                };

                                tokio::select! {
                                    _ = token_clone.cancelled() => {
                                    }
                                    res = worker.fetch_segment(meta_id, segment, Some(progress_tx), Some(storage_client.clone()), throttler_clone.clone()) => {
                                        // Ensure all progress events are forwarded before finishing the range
                                        let _ = progress_handle.await;

                                        match res {
                                            Ok(piece) => {
                                                let _ = storage_client.submit_write(
                                                    meta_id,
                                                    piece.segment,
                                                    piece.data,
                                                    None,
                                                    None,
                                                ).await;
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
                                let worker = orchestrator_handle
                                    .build_worker_builder(uri, tenant_id_clone)
                                    .build_ftp();
                                let segment = Segment {
                                    offset: range.start,
                                    length: range.length(),
                                };

                                tokio::select! {
                                    _ = token_clone.cancelled() => {
                                    }
                                    res = worker.fetch_segment(meta_id, segment, Some(progress_tx), Some(storage_client.clone()), throttler_clone.clone()) => {
                                        match res {
                                            Ok(piece) => {
                                                let _ = storage_client.submit_write(
                                                    meta_id,
                                                    piece.segment,
                                                    piece.data,
                                                    None,
                                                    None,
                                                ).await;
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
                            TaskType::S3 => {
                                let worker = orchestrator_handle
                                    .build_worker_builder(uri, tenant_id_clone)
                                    .build_s3();
                                let segment = Segment {
                                    offset: range.start,
                                    length: range.length(),
                                };

                                tokio::select! {
                                    _ = token_clone.cancelled() => {
                                    }
                                    res = worker.fetch_segment(meta_id, segment, Some(progress_tx), Some(storage_client.clone()), throttler_clone.clone()) => {
                                        let _ = progress_handle.await;
                                        match res {
                                            Ok(piece) => {
                                                let _ = storage_client.submit_write(
                                                    meta_id,
                                                    piece.segment,
                                                    piece.data,
                                                    None,
                                                    None,
                                                ).await;
                                                let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, range)).await;
                                            }
                                            Err(e) => {
                                                debug!(%meta_id, %sub_id, error = %e, "S3 range fetch failed");
                                                let _ = subtask_tx.send(crate::orchestrator::SubTaskEvent::Failed(meta_id, sub_id, e.to_string())).await;
                                            }
                                        }
                                    }
                                }
                            }
                            TaskType::GDrive => {
                                let worker = orchestrator_handle
                                    .build_worker_builder(uri, tenant_id_clone)
                                    .build_gdrive();
                                let segment = Segment {
                                    offset: range.start,
                                    length: range.length(),
                                };

                                tokio::select! {
                                    _ = token_clone.cancelled() => {
                                    }
                                    res = worker.fetch_segment(meta_id, segment, Some(progress_tx), Some(storage_client.clone()), throttler_clone.clone()) => {
                                        let _ = progress_handle.await;
                                        match res {
                                            Ok(piece) => {
                                                let _ = storage_client.submit_write(
                                                    meta_id,
                                                    piece.segment,
                                                    piece.data,
                                                    None,
                                                    None,
                                                ).await;
                                                let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, range)).await;
                                            }
                                            Err(e) => {
                                                debug!(%meta_id, %sub_id, error = %e, "GDrive range fetch failed");
                                                let _ = subtask_tx.send(crate::orchestrator::SubTaskEvent::Failed(meta_id, sub_id, e.to_string())).await;
                                            }
                                        }
                                    }
                                }
                            }
                            TaskType::Nntp => {
                                #[cfg(feature = "nntp")]
                                let worker = orchestrator_handle
                                    .build_worker_builder(uri, tenant_id_clone)
                                    .build_nntp();
                                let segment = Segment {
                                    offset: range.start,
                                    length: range.length(),
                                };

                                tokio::select! {
                                    _ = token_clone.cancelled() => {}
                                    res = async {
                                        #[cfg(feature = "nntp")] {
                                            worker.fetch_segment(meta_id, segment, Some(progress_tx), Some(storage_client.clone()), throttler_clone.clone()).await
                                        }
                                        #[cfg(not(feature = "nntp"))] {
                                            let _ = (&meta_id, &progress_tx, &storage_client, &throttler_clone, &segment);
                                            Err::<crate::worker::PieceData, crate::Error>(crate::Error::Protocol("NNTP feature not enabled".to_string()))
                                        }
                                    } => {
                                        let _ = progress_handle.await;
                                        match res {
                                            Ok(piece) => {
                                                let _ = storage_client.submit_write(
                                                    meta_id,
                                                    piece.segment,
                                                    piece.data,
                                                    None,
                                                    None,
                                                ).await;
                                                let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, range)).await;
                                            }
                                            Err(e) => {
                                                debug!(%meta_id, %sub_id, error = %e, "NNTP range fetch failed");
                                                let _ = subtask_tx.send(crate::orchestrator::SubTaskEvent::Failed(meta_id, sub_id, e.to_string())).await;
                                            }
                                        }
                                    }
                                }
                            }
                            TaskType::BitTorrent => {}
                        }
                    });
                } else {
                    break;
                }
            }
        }
        Ok(())
    }
}
