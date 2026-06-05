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

    pub(crate) async fn handle_refresh(&mut self, id: TaskId) -> Result<()> {
        let meta_task = self.tasks.get(&id).ok_or(crate::Error::TaskNotFound(id))?;

        // We only support refreshing HTTP tasks
        let http_subtasks: Vec<_> = meta_task
            .subtasks
            .iter()
            .filter(|s| s.task_type == crate::task::TaskType::Http)
            .cloned()
            .collect();

        if http_subtasks.is_empty() {
            return Err(crate::Error::Protocol(
                "Refresh is only supported for HTTP tasks".to_string(),
            ));
        }

        let config_clone = self.config.clone();
        let subtask_tx = self.subtask_tx.clone();
        let client_pool = self.client_pool.clone();
        let provider_clone = self.credential_provider.clone();
        let dns_resolver = self.dns_resolver.clone();
        let hsts_cache = self.hsts_cache.clone();
        let alt_svc_cache = self.alt_svc_cache.clone();

        let etag = meta_task.etag.clone();
        let last_modified = meta_task.last_modified.clone();
        let local_addr = self.resolve_local_addr();

        // Spawn a metadata check for each HTTP subtask
        for sub_task in http_subtasks {
            let sub_id = sub_task.id;
            let uri = sub_task.uri.clone();
            let subtask_tx_clone = subtask_tx.clone();
            let client_pool_clone = client_pool.clone();
            let provider_clone_c = provider_clone.clone();
            let dns_resolver_c = dns_resolver.clone();
            let hsts_cache_c = hsts_cache.clone();
            let alt_svc_cache_c = alt_svc_cache.clone();
            let etag_val = etag.clone();
            let lm_val = last_modified.clone();
            let config_clone_c = config_clone.clone();

            tokio::spawn(async move {
                let config = config_clone_c.load();
                let worker = crate::worker::WorkerBuilder::new(uri)
                    .local_addr(local_addr)
                    .dns_resolver(dns_resolver_c)
                    .user_agent(Some(config.network.user_agent.clone()))
                    .connect_timeout(Some(config.network.connect_timeout_secs))
                    .proxy(config.network.proxy.clone())
                    .retry_count(config.network.http_retry_count)
                    .retry_delay_secs(config.network.http_retry_delay_secs)
                    .credential_provider(provider_clone_c)
                    .hsts_cache(hsts_cache_c)
                    .alt_svc_cache(alt_svc_cache_c)
                    .client_pool(client_pool_clone)
                    .if_none_match(etag_val)
                    .if_modified_since(lm_val)
                    .build_http();

                match worker.resolve_metadata().await {
                    Ok(m) => {
                        let _ = subtask_tx_clone
                            .send(crate::orchestrator::SubTaskEvent::RefreshMatured(
                                id, sub_id, m,
                            ))
                            .await;
                    }
                    Err(crate::Error::NotModified) => {
                        let _ = subtask_tx_clone
                            .send(crate::orchestrator::SubTaskEvent::RefreshNotModified(
                                id, sub_id,
                            ))
                            .await;
                    }
                    Err(e) => {
                        let _ = subtask_tx_clone
                            .send(crate::orchestrator::SubTaskEvent::RefreshFailed(
                                id,
                                sub_id,
                                e.to_string(),
                            ))
                            .await;
                    }
                }
            });
        }

        Ok(())
    }
}
