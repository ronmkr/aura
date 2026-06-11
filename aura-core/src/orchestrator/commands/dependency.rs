use crate::orchestrator::Orchestrator;
use crate::task::DownloadPhase;
use crate::{Result, TaskId};
use tracing::info;

impl Orchestrator {
    pub(crate) async fn preempt_resources_for_high_priority(&mut self, new_task_id: TaskId) {
        let config = self.config.load();
        let max_concurrent = config.bandwidth.max_concurrent_downloads;
        let min_concurrency = config.bandwidth.min_connections_per_task;

        // Verify if the triggering task is Priority 0 and active
        let is_prio_0 = self
            .tasks
            .get(&new_task_id)
            .map(|t| {
                t.priority == 0
                    && (t.phase == DownloadPhase::Downloading
                        || t.phase == DownloadPhase::MetadataExchange
                        || t.phase == DownloadPhase::Verifying)
            })
            .unwrap_or(false);

        if !is_prio_0 {
            return;
        }

        info!(%new_task_id, "Triggering high-priority resource preemption checks");

        // 1. Pause/preempt the lowest-priority active task if we exceed max_concurrent_downloads
        let active_tasks: Vec<(TaskId, u32)> = self
            .tasks
            .iter()
            .filter(|(&id, t)| {
                id != new_task_id
                    && (t.phase == DownloadPhase::Downloading
                        || t.phase == DownloadPhase::MetadataExchange
                        || t.phase == DownloadPhase::Verifying)
            })
            .map(|(&id, t)| (id, t.priority))
            .collect();

        // Count of active downloads including the new one
        let active_count = active_tasks.len() + 1;

        if active_count > max_concurrent {
            // Find lowest priority (highest priority number)
            if let Some(&(lowest_id, lowest_priority)) =
                active_tasks.iter().max_by_key(|&&(_, p)| p)
            {
                if lowest_priority > 0 {
                    info!(%lowest_id, %lowest_priority, "Preempting task to Waiting state due to high-priority task concurrency limits");
                    let _ = self.handle_pause(lowest_id).await;
                    if let Some(task) = self.tasks.get_mut(&lowest_id) {
                        task.phase = DownloadPhase::Waiting;
                        let _ = self.save_task(lowest_id).await;
                    }
                }
            }
        }

        // 2. Scale down target concurrency of lower priority tasks to min_connections_per_task
        // and cancel excess connections by pausing and resuming them.
        let lower_prio_tasks: Vec<TaskId> = self
            .tasks
            .iter()
            .filter(|(&id, t)| {
                id != new_task_id
                    && t.priority > 0
                    && (t.phase == DownloadPhase::Downloading
                        || t.phase == DownloadPhase::MetadataExchange)
            })
            .map(|(&id, _)| id)
            .collect();

        for id in lower_prio_tasks {
            let mut needs_resize = false;
            if let Some(task) = self.tasks.get(&id) {
                for sub in &task.subtasks {
                    if sub.target_concurrency > min_concurrency {
                        needs_resize = true;
                        break;
                    }
                }
            }

            if needs_resize {
                info!(%id, "Scaling down target concurrency to minimum and recycling connections");
                let _ = self.handle_pause(id).await;

                if let Some(task) = self.tasks.get_mut(&id) {
                    for sub in &mut task.subtasks {
                        if sub.target_concurrency > min_concurrency {
                            sub.target_concurrency = min_concurrency;
                        }
                    }
                    task.phase = DownloadPhase::Downloading;
                    let _ = self.save_task(id).await;

                    let token = tokio_util::sync::CancellationToken::new();
                    self.cancellation_tokens.insert(id, token.clone());
                    let _ = self.start_task_loops_with_bitfield(id, token, None).await;
                }
            }
        }
    }

    pub(crate) async fn check_waiting_tasks(&mut self) {
        let mut to_start = Vec::new();

        for (&id, task) in &self.tasks {
            if task.phase == DownloadPhase::Waiting {
                let mut all_parents_complete = true;
                for &parent_id in &task.depends_on {
                    if let Some(parent) = self.tasks.get(&parent_id) {
                        if parent.phase != DownloadPhase::Complete {
                            all_parents_complete = false;
                            break;
                        }
                    } else {
                        all_parents_complete = false;
                        break;
                    }
                }

                if all_parents_complete {
                    to_start.push(id);
                }
            }
        }

        for id in to_start {
            info!(%id, "Unblocking waiting task as all dependencies are now complete");
            if let Some(task) = self.tasks.get_mut(&id) {
                task.phase = DownloadPhase::Downloading;
                let _ = self.save_task(id).await;

                let token = tokio_util::sync::CancellationToken::new();
                self.cancellation_tokens.insert(id, token.clone());
                let _ = self.start_task_loops_with_bitfield(id, token, None).await;

                let _ = self.preempt_resources_for_high_priority(id).await;
            }
        }
    }

    pub(crate) async fn handle_change_option(
        &mut self,
        id: TaskId,
        priority: Option<u32>,
        depends_on: Option<Vec<TaskId>>,
        seed_ratio: Option<f32>,
        seed_time: Option<u32>,
        streaming_mode: Option<bool>,
    ) -> Result<()> {
        let mut priority_changed = false;
        let mut depends_changed = false;
        let mut seeding_changed = false;
        let mut streaming_changed = false;

        if let Some(p) = priority {
            if let Some(task) = self.tasks.get_mut(&id) {
                if task.priority != p {
                    info!(%id, from = %task.priority, to = %p, "Updating task priority");
                    task.priority = p;
                    priority_changed = true;
                }
            }
            let t = self
                .tasks
                .get(&id)
                .and_then(|t| t.tenant_id.as_ref())
                .and_then(|tid| self.tenants.get(tid))
                .map(|c| &c.throttler)
                .unwrap_or(&self.throttler);
            t.update_task_priority(id, p).await;
        }

        if let Some(ref deps) = depends_on {
            let original_deps = self.tasks.get(&id).map(|t| t.depends_on.clone());
            if let Some(task) = self.tasks.get_mut(&id) {
                if task.depends_on != *deps {
                    info!(%id, "Updating task dependencies");
                    task.depends_on = deps.clone();
                    depends_changed = true;
                }
            }

            if depends_changed && self.has_cycle() {
                // Revert dependencies
                if let Some(orig) = original_deps {
                    if let Some(task) = self.tasks.get_mut(&id) {
                        task.depends_on = orig;
                    }
                }
                return Err(crate::Error::Engine(
                    "Dependency cycle detected".to_string(),
                ));
            }
        }

        if let Some(ratio) = seed_ratio {
            if let Some(task) = self.tasks.get_mut(&id) {
                task.seed_ratio_override = Some(ratio);
            }
            if let Some(bt) = self.get_bt_task(id) {
                let mut seed_ratio_guard = bt.state.seed_ratio.lock().unwrap();
                if *seed_ratio_guard != Some(ratio) {
                    info!(%id, from = ?*seed_ratio_guard, to = ?Some(ratio), "Updating task seed_ratio override");
                    *seed_ratio_guard = Some(ratio);
                    seeding_changed = true;
                }
            }
        }

        if let Some(time) = seed_time {
            if let Some(task) = self.tasks.get_mut(&id) {
                task.seed_time_override = Some(time);
            }
            if let Some(bt) = self.get_bt_task(id) {
                let mut seed_time_guard = bt.state.seed_time.lock().unwrap();
                if *seed_time_guard != Some(time) {
                    info!(%id, from = ?*seed_time_guard, to = ?Some(time), "Updating task seed_time override");
                    *seed_time_guard = Some(time);
                    seeding_changed = true;
                }
            }
        }

        if let Some(mode) = streaming_mode {
            if let Some(task) = self.tasks.get_mut(&id) {
                if task.streaming_mode != mode {
                    info!(%id, from = %task.streaming_mode, to = %mode, "Updating task streaming_mode");
                    task.streaming_mode = mode;
                    streaming_changed = true;
                    if let Some(bt) = self.get_bt_task(id) {
                        bt.state
                            .streaming_mode
                            .store(mode, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        }

        if priority_changed || depends_changed || seeding_changed || streaming_changed {
            let _ = self.save_task(id).await;
        }

        // Handle dependency condition changes
        if depends_changed {
            let is_blocked = {
                let mut blocked = false;
                if let Some(task) = self.tasks.get(&id) {
                    for &parent_id in &task.depends_on {
                        if let Some(parent) = self.tasks.get(&parent_id) {
                            if parent.phase != DownloadPhase::Complete {
                                blocked = true;
                                break;
                            }
                        } else {
                            blocked = true;
                            break;
                        }
                    }
                }
                blocked
            };

            if is_blocked {
                if let Some(task) = self.tasks.get(&id) {
                    if task.phase == DownloadPhase::Downloading
                        || task.phase == DownloadPhase::MetadataExchange
                    {
                        info!(%id, "Moving task to Waiting phase due to new unsatisfied dependency");
                        let _ = self.handle_pause(id).await;
                        if let Some(t) = self.tasks.get_mut(&id) {
                            t.phase = DownloadPhase::Waiting;
                            let _ = self.save_task(id).await;
                        }
                    }
                }
            } else {
                let is_waiting = self
                    .tasks
                    .get(&id)
                    .map(|t| t.phase == DownloadPhase::Waiting)
                    .unwrap_or(false);
                if is_waiting {
                    info!(%id, "Starting previously waiting task as its dependencies are satisfied");
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

        if priority_changed {
            if let Some(p) = priority {
                if p == 0 {
                    self.preempt_resources_for_high_priority(id).await;
                }
            }
        }

        Ok(())
    }
}
