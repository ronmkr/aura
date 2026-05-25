use super::{Command, Event, Orchestrator, SubTaskEvent};
use crate::task::{DownloadPhase, MetaTask, TaskType};
use crate::{Error, Result, TaskId};
use std::sync::Arc;
use tracing::{info, warn};

impl Orchestrator {
    pub(crate) async fn handle_command(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::AddTask {
                id,
                name,
                sources,
                checksum,
                priority,
                streaming_mode,
            } => {
                self.handle_add_task(id, name, sources, checksum, priority, streaming_mode)
                    .await?;
            }
            Command::Pause(id) => {
                self.handle_pause(id).await?;
            }
            Command::Resume(id) => {
                self.handle_resume(id).await?;
            }
            Command::Remove(id) => {
                let _ = self.handle_pause(id).await;
                self.throttler.unregister_task(id).await;
                self.tasks.remove(&id);
            }
            Command::ListActive(reply_tx) => {
                let active: Vec<MetaTask> = self.tasks.values().cloned().collect();
                let _ = reply_tx.send(active).await;
            }
            Command::GetConfig(reply_tx) => {
                let config = self.config.load().clone();
                let _ = reply_tx.send(config).await;
            }
            Command::ReloadConfig(new_config, resp_tx) => {
                info!("Reloading configuration");
                self.throttler
                    .set_global_download_limit(new_config.bandwidth.global_download_limit);
                self.throttler
                    .set_global_upload_limit(new_config.bandwidth.global_upload_limit);

                // Update VPN provider if changed
                if self.config.load().vpn != new_config.vpn
                    || self.config.load().network.interface != new_config.network.interface
                {
                    self.update_vpn_provider(&new_config);
                }

                self.hook_manager.update_config(new_config.hooks.clone());

                // Update CredentialProvider if paths changed
                if self.config.load().credentials.netrc_path != new_config.credentials.netrc_path
                    || self.config.load().credentials.cookie_file
                        != new_config.credentials.cookie_file
                {
                    info!("Reloading credentials");
                    let mut new_provider = crate::config::credentials::CredentialProvider::new();
                    if let Some(ref netrc) = new_config.credentials.netrc_path {
                        if let Err(e) = new_provider.load_netrc(netrc) {
                            warn!("Failed to reload .netrc from {}: {}", netrc, e);
                        }
                    }
                    if let Some(ref cookie_file) = new_config.credentials.cookie_file {
                        if let Err(e) = new_provider.load_cookies(cookie_file) {
                            warn!("Failed to reload cookies from {}: {}", cookie_file, e);
                        }
                    }
                    self.credential_provider = Arc::new(new_provider);
                }

                // Update DNS resolver if changed
                if self.config.load().network.dns_resolver != new_config.network.dns_resolver {
                    info!("Reloading DNS resolver");
                    match crate::net_util::create_resolver(&new_config.network.dns_resolver).await {
                        Ok(new_resolver) => {
                            self.dns_resolver = Arc::new(new_resolver);
                        }
                        Err(e) => {
                            warn!("Failed to reload DNS resolver: {}", e);
                        }
                    }
                }

                self.config.store(new_config);
                let _ = resp_tx.send(());
            }
            Command::KillSwitch => {
                let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                for id in ids {
                    let _ = self.handle_pause(id).await;
                }
            }
            Command::Shutdown => {
                info!("Orchestrator shutting down");
                // We return Err to break the loop in run() or just a special signal
                // For now, run() will see the end of command_rx if we drop it,
                // but explicit shutdown is better.
                return Err(Error::Engine("Shutting down".to_string()));
            }
            Command::RetrySubtask(meta_id, sub_id) => {
                self.handle_retry_subtask(meta_id, sub_id).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn handle_add_task(
        &mut self,
        id: TaskId,
        name: String,
        sources: Vec<(String, crate::task::TaskType)>,
        checksum: Option<crate::Checksum>,
        priority: u32,
        streaming_mode: bool,
    ) -> Result<()> {
        // Enforce mandatory tunnel
        self.verify_vpn_connectivity().await?;

        info!(%id, %name, "Adding MetaTask with {} sources", sources.len());

        let config = self.config.load();
        let download_dir = &config.storage.download_dir;
        let control_path = std::path::Path::new(download_dir).join(format!("{}.aura", name));

        let (mut meta_task, loaded_bitfield) = if let Ok(data) = std::fs::read(&control_path) {
            match serde_json::from_slice::<crate::task::TaskState>(&data) {
                Ok(state) => {
                    info!(%id, "Resuming task from control file {:?}", control_path);
                    let bitfield = state.bitfield.clone();
                    let mut mt = MetaTask::from_state(state);
                    mt.id = id; // Update to the new ID
                    (mt, bitfield)
                }
                Err(e) => {
                    tracing::warn!(%id, "Failed to parse control file {:?}: {}. Starting fresh.", control_path, e);
                    (MetaTask::new(id, name, 0), None)
                }
            }
        } else {
            (MetaTask::new(id, name, 0), None)
        };

        if let Some(c) = checksum {
            meta_task.checksum = Some(c);
        }
        meta_task.priority = priority;
        meta_task.streaming_mode = streaming_mode;

        if meta_task.subtasks.is_empty() {
            for (uri, ttype) in sources {
                if uri.ends_with(".metalink") || uri.ends_with(".meta4") {
                    if let Ok(data) = std::fs::read(&uri) {
                        if let Ok(ml) = crate::metalink::Metalink::parse(&data) {
                            for file in ml.files {
                                if meta_task.name == "unnamed" || meta_task.name.is_empty() {
                                    meta_task.name = file.name.clone();
                                }
                                if meta_task.total_length == 0 {
                                    if let Some(size) = file.size {
                                        meta_task.total_length = size;
                                        meta_task.generate_ranges(16);
                                    }
                                }
                                for res in file.resources {
                                    let res_ttype = match res.protocol.to_lowercase().as_str() {
                                        "ftp" => crate::task::TaskType::Ftp,
                                        _ => crate::task::TaskType::Http,
                                    };
                                    tracing::debug!(uri = %res.uri, protocol = %res.protocol, resolved = ?res_ttype, "Adding Metalink subtask");
                                    meta_task.add_subtask(res.uri, res_ttype);
                                }
                            }
                            continue;
                        }
                    }
                }
                meta_task.add_subtask(uri, ttype);
            }
        }

        let token = tokio_util::sync::CancellationToken::new();
        self.cancellation_tokens.insert(id, token.clone());

        let config = self.config.load();
        self.throttler
            .register_task(
                id,
                config.bandwidth.per_task_download_limit,
                config.bandwidth.per_task_upload_limit,
            )
            .await;

        let path = std::path::Path::new(download_dir).join(&meta_task.name);
        let _ = self
            .storage_tx
            .send(crate::storage::StorageRequest::RegisterTask {
                task_id: id,
                path,
                total_length: meta_task.total_length,
                checksum: meta_task.checksum.clone(),
            })
            .await;

        self.tasks.insert(id, meta_task);
        self.start_task_loops_with_bitfield(id, token, loaded_bitfield)
            .await?;

        let event = Event::TaskAdded(id);
        let _ = self.event_tx.send(event.clone());
        self.hook_manager.handle_event(&event).await;
        Ok(())
    }

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

    pub(crate) async fn handle_retry_subtask(&mut self, id: TaskId, sub_id: TaskId) -> Result<()> {
        let meta_task = self.tasks.get_mut(&id).ok_or(Error::TaskNotFound(id))?;

        let token = self
            .cancellation_tokens
            .get(&id)
            .cloned()
            .ok_or_else(|| Error::Engine("No cancellation token for task".to_string()))?;

        if token.is_cancelled() {
            return Ok(());
        }

        if let Some(sub_task) = meta_task.subtasks.iter_mut().find(|s| s.id == sub_id) {
            if sub_task.phase != DownloadPhase::Degraded || sub_task.active {
                return Ok(());
            }

            let uri = sub_task.uri.clone();
            let ttype = sub_task.task_type.clone();

            if meta_task.blacklisted_uris.contains(&uri) {
                sub_task.phase = DownloadPhase::Error;
                sub_task.active = false;
                return Ok(());
            }

            sub_task.active = true;

            let subtask_tx = self.subtask_tx.clone();
            let config_clone = self.config.clone();
            let pool = self.pool.clone();
            let local_addr = self.resolve_local_addr();
            let provider_clone = self.credential_provider.clone();
            let dns_resolver = self.dns_resolver.clone();

            tracing::info!(%id, %sub_id, %uri, "Retrying/Self-healing Degraded subtask");

            tokio::spawn(async move {
                let config = config_clone.load();
                match ttype {
                    TaskType::Http => {
                        let worker = crate::worker::WorkerBuilder::new(uri)
                            .local_addr(local_addr)
                            .dns_resolver(dns_resolver)
                            .user_agent(Some(config.network.user_agent.clone()))
                            .connect_timeout(Some(config.network.connect_timeout_secs))
                            .proxy(config.network.proxy.clone())
                            .with_pool(pool.clone())
                            .retry_count(config.network.http_retry_count)
                            .retry_delay_secs(config.network.http_retry_delay_secs)
                            .credential_provider(provider_clone)
                            .build_http();
                        match worker.resolve_metadata().await {
                            Ok(m) => {
                                let _ = subtask_tx.send(SubTaskEvent::Matured(id, sub_id, m)).await;
                            }
                            Err(e) => {
                                let _ = subtask_tx
                                    .send(crate::orchestrator::SubTaskEvent::Failed(
                                        id,
                                        sub_id,
                                        e.to_string(),
                                    ))
                                    .await;
                            }
                        }
                    }
                    TaskType::Ftp => {
                        let worker = crate::worker::WorkerBuilder::new(uri)
                            .local_addr(local_addr)
                            .with_pool(pool.clone())
                            .credential_provider(provider_clone)
                            .build_ftp();
                        match worker.resolve_metadata().await {
                            Ok(m) => {
                                let _ = subtask_tx.send(SubTaskEvent::Matured(id, sub_id, m)).await;
                            }
                            Err(e) => {
                                let _ = subtask_tx
                                    .send(crate::orchestrator::SubTaskEvent::Failed(
                                        id,
                                        sub_id,
                                        e.to_string(),
                                    ))
                                    .await;
                            }
                        }
                    }
                    TaskType::BitTorrent => {}
                }
            });
        }

        Ok(())
    }
}
