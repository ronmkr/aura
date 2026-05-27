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
        let event = Event::TaskPaused(id);
        let _ = self.event_tx.send(event.clone());
        self.hook_manager.handle_event(&event).await;
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

                let event = Event::TaskResumed(id);
                let _ = self.event_tx.send(event.clone());
                self.hook_manager.handle_event(&event).await;
            }
        }
        Ok(())
    }

    pub(crate) async fn check_seed_limits(&mut self) {
        let config = self.config.load();
        let target_ratio = config.bittorrent.seed_ratio;
        let target_time = config.bittorrent.seed_time_mins as i64;

        let mut to_pause = Vec::new();
        for (id, task) in &self.tasks {
            if task.phase == crate::task::DownloadPhase::Complete {
                // Check Ratio
                if target_ratio > 0.0 {
                    let current_ratio = if task.completed_length > 0 {
                        task.uploaded_length as f64 / task.completed_length as f64
                    } else {
                        0.0
                    };
                    if current_ratio >= target_ratio as f64 {
                        tracing::info!(%id, current_ratio, target_ratio, "Seed ratio reached, pausing task");
                        to_pause.push(*id);
                        continue;
                    }
                }

                // Check Time
                if target_time > 0 {
                    if let Some(start_time) = task.seeding_start_time {
                        let elapsed = chrono::Utc::now() - start_time;
                        if elapsed.num_minutes() >= target_time {
                            tracing::info!(%id, elapsed_mins = elapsed.num_minutes(), target_time, "Seed time limit reached, pausing task");
                            to_pause.push(*id);
                        }
                    }
                }
            }
        }

        for id in to_pause {
            let _ = self.handle_pause(id).await;
        }
    }
}
