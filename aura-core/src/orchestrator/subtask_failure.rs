use super::{ErrorSeverity, Event, Orchestrator, SubTaskEvent};
use crate::task::DownloadPhase;
use crate::{Result, TaskId};
use tracing::{error, info, warn};

impl Orchestrator {
    pub(crate) async fn handle_subtask_failed(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        err: String,
    ) -> Result<()> {
        self.worker_cancellation_tokens.remove(&sub_id);
        info!(%meta_id, %sub_id, %err, "Subtask failed");

        let severity = self.policy_manager.classify(&err);

        if let Some(task) = self.tasks.get_mut(&meta_id) {
            if let Some(sub) = task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                sub.active = false;

                match severity {
                    ErrorSeverity::Engine => {
                        warn!(%meta_id, "Engine level error detected: {}; pausing task", err);
                        sub.phase = DownloadPhase::Paused;
                        let _ = self.handle_pause(meta_id).await;
                        if let Some(t) = self.tasks.get_mut(&meta_id) {
                            t.phase = DownloadPhase::Paused;
                            let _ = self.save_task(meta_id).await;
                        }
                        let _ = self.event_tx.send(Event::TaskPaused(meta_id));
                        return Ok(());
                    }
                    ErrorSeverity::Task => {
                        sub.phase = DownloadPhase::Error;
                        error!(%meta_id, %sub_id, "Task level error; mirror permanently failed: {}", err);
                        task.blacklisted_uris.push(sub.uri.clone());
                    }
                    ErrorSeverity::Worker => {
                        sub.retry_count += 1;
                        let config = self.config.load();
                        let max_retries = config.network.http_retry_count;
                        let delay_base = config.network.http_retry_delay_secs;

                        if sub.retry_count < max_retries {
                            sub.phase = DownloadPhase::Degraded;
                            warn!(%meta_id, %sub_id, count = sub.retry_count, "Mirror degraded, recycling ranges");

                            let subtask_tx = self.subtask_tx.clone();
                            let retry_delay = self
                                .policy_manager
                                .get_retry_delay(sub.retry_count, delay_base);
                            tokio::spawn(async move {
                                tokio::time::sleep(retry_delay).await;
                                let _ = subtask_tx.send(SubTaskEvent::Retry(meta_id, sub_id)).await;
                            });
                        } else {
                            sub.phase = DownloadPhase::Error;
                            error!(%meta_id, %sub_id, "Worker error escalated to Task; permanently failed after {} retries", max_retries);
                            task.blacklisted_uris.push(sub.uri.clone());
                        }
                    }
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
                self.notification_service.notify_error(&task.name, &err);
                let _ = self.event_tx.send(Event::TaskError {
                    id: meta_id,
                    message: err,
                });
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
        Ok(())
    }
}
