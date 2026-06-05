use crate::orchestrator::{Event, Orchestrator};
use crate::task::DownloadPhase;
use crate::{Result, TaskId};
use tracing::info;

impl Orchestrator {
    pub(crate) async fn handle_pause(&mut self, id: TaskId) -> Result<()> {
        if let Some(task) = self.tasks.get_mut(&id) {
            if task.phase != DownloadPhase::Paused && task.phase != DownloadPhase::Error {
                info!(%id, "Pausing task");
                task.phase = DownloadPhase::Paused;
            }

            if let Some(token) = self.cancellation_tokens.remove(&id) {
                token.cancel();
            }

            for sub in &task.subtasks {
                if let Some(w_token) = self.worker_cancellation_tokens.remove(&sub.id) {
                    w_token.cancel();
                }
            }

            // Recycle in-flight ranges
            let in_flight = std::mem::take(&mut task.in_flight_ranges);
            for (_sub_id, range) in in_flight {
                task.pending_ranges.push(range);
            }

            let _ = self.event_tx.send(Event::TaskProgress {
                id,
                completed_bytes: task.completed_length,
                uploaded_bytes: task.uploaded_length,
                total_bytes: task.total_length,
            });
        }
        let _ = self.save_task(id).await;
        let _ = self.event_tx.send(Event::TaskPaused(id));
        Ok(())
    }

    pub(crate) async fn handle_resume(&mut self, id: TaskId) -> Result<()> {
        // Enforce mandatory tunnel
        self.verify_vpn_connectivity().await?;

        if let Some(task) = self.tasks.get_mut(&id) {
            if task.phase == DownloadPhase::Paused {
                info!(%id, "Resuming task");
                task.phase = DownloadPhase::Downloading;
                let token = tokio_util::sync::CancellationToken::new();
                self.cancellation_tokens.insert(id, token.clone());

                // For resume, we don't reload bitfield from file here,
                // we assume the in-memory one is correct or will be synced.
                self.start_task_loops_with_bitfield(id, token, None).await?;

                let _ = self.event_tx.send(Event::TaskResumed(id));
            }
        }
        Ok(())
    }

    pub(crate) async fn check_seed_limits(&mut self) {
        let config = self.config.load();
        let global_ratio = config.bittorrent.seeding.min_ratio;
        let global_time_secs = config.bittorrent.seeding.max_seeding_time_secs;
        let global_stop_on_either = config.bittorrent.seeding.stop_on_either;

        let mut to_stop = Vec::new();
        for (id, task) in &self.tasks {
            if task.phase == crate::task::DownloadPhase::Complete {
                let is_bittorrent = task
                    .subtasks
                    .iter()
                    .any(|sub| sub.task_type == crate::task::TaskType::BitTorrent);
                if !is_bittorrent {
                    continue;
                }

                let target_ratio = task.seed_ratio.unwrap_or(global_ratio);
                let target_time_secs = task
                    .seed_time
                    .map(|mins| mins as u64 * 60)
                    .unwrap_or(global_time_secs);

                let ratio_limit_active = target_ratio > 0.0;
                let time_limit_active = target_time_secs > 0;

                if !ratio_limit_active && !time_limit_active {
                    continue;
                }

                let ratio_reached = if ratio_limit_active {
                    let current_ratio = if task.total_length > 0 {
                        task.uploaded_length as f64 / task.total_length as f64
                    } else {
                        0.0
                    };
                    current_ratio >= target_ratio as f64
                } else {
                    false
                };

                let time_reached = if time_limit_active {
                    if let Some(start_time) = task.seeding_start_time {
                        let elapsed_secs =
                            (chrono::Utc::now() - start_time).num_seconds().max(0) as u64;
                        elapsed_secs >= target_time_secs
                    } else {
                        false
                    }
                } else {
                    false
                };

                let should_stop = match (ratio_limit_active, time_limit_active) {
                    (true, true) => {
                        if global_stop_on_either {
                            ratio_reached || time_reached
                        } else {
                            ratio_reached && time_reached
                        }
                    }
                    (true, false) => ratio_reached,
                    (false, true) => time_reached,
                    (false, false) => false,
                };

                if should_stop {
                    let reason = if ratio_reached
                        && (!time_reached || global_stop_on_either || !time_limit_active)
                    {
                        crate::SeedingCompleteReason::RatioReached
                    } else {
                        crate::SeedingCompleteReason::TimeExpired
                    };
                    to_stop.push((*id, reason));
                }
            }
        }

        for (id, reason) in to_stop {
            tracing::info!(%id, ?reason, "Seeding limit reached, stopping task");
            let _ = self.handle_pause(id).await;
            let _ = self.event_tx.send(Event::SeedingComplete { id, reason });
        }
    }
}
