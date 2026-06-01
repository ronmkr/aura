use crate::orchestrator::{Event, Orchestrator};
use crate::task::MetaTask;
use crate::{Result, TaskId};
use tracing::info;

impl Orchestrator {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn handle_add_task(
        &mut self,
        id: TaskId,
        tenant_id: Option<crate::TenantId>,
        name: String,
        sources: Vec<(String, crate::task::TaskType)>,
        checksum: Option<crate::Checksum>,
        priority: u32,
        streaming_mode: bool,
        depends_on: Vec<TaskId>,
        follow_on: Option<crate::task::FollowOnAction>,
    ) -> Result<()> {
        // Enforce mandatory tunnel
        self.verify_vpn_connectivity().await?;

        // Enforce per-tenant task count quotas
        if let Some(ref tid) = tenant_id {
            if let Some(ctx) = self.tenants.get(tid) {
                if let Some(max) = ctx.max_tasks {
                    let active_tasks = self
                        .tasks
                        .values()
                        .filter(|t| t.tenant_id == Some(tid.clone()))
                        .count();
                    if active_tasks >= max {
                        return Err(crate::Error::Engine(format!(
                            "Tenant task limit reached: {}",
                            max
                        )));
                    }
                }
            }
        }

        info!(%id, %name, "Adding MetaTask with {} sources", sources.len());

        let config = self.config.load();
        let base_dir = if let Some(ref tid) = tenant_id {
            if let Some(ctx) = self.tenants.get(tid) {
                if let Some(ref root) = ctx.disk_path_root {
                    root.clone()
                } else {
                    std::path::PathBuf::from(&config.storage.download_dir)
                }
            } else {
                std::path::PathBuf::from(&config.storage.download_dir)
            }
        } else {
            std::path::PathBuf::from(&config.storage.download_dir)
        };
        let control_path = base_dir.join(format!("{}.aura", name));

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
        meta_task.tenant_id = tenant_id.clone();
        meta_task.follow_on = follow_on.clone();
        meta_task.depends_on = depends_on.clone();

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
                                        meta_task.generate_ranges(128);
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

        meta_task.depends_on = depends_on.clone();

        // Perform cycle detection!
        let original_task = self.tasks.insert(id, meta_task.clone());
        if self.has_cycle() {
            if let Some(orig) = original_task {
                self.tasks.insert(id, orig);
            } else {
                self.tasks.remove(&id);
            }
            return Err(crate::Error::Engine(
                "Dependency cycle detected".to_string(),
            ));
        }
        self.tasks.remove(&id);

        let token = tokio_util::sync::CancellationToken::new();
        self.cancellation_tokens.insert(id, token.clone());

        let config = self.config.load();
        let throttler = if let Some(ref tid) = meta_task.tenant_id {
            if let Some(ctx) = self.tenants.get(tid) {
                ctx.throttler.clone()
            } else {
                self.throttler.clone()
            }
        } else {
            self.throttler.clone()
        };

        throttler
            .register_task(
                id,
                config.bandwidth.per_task_download_limit,
                config.bandwidth.per_task_upload_limit,
                meta_task.priority,
            )
            .await;

        let base_dir = if let Some(ref tid) = meta_task.tenant_id {
            if let Some(ctx) = self.tenants.get(tid) {
                if let Some(ref root) = ctx.disk_path_root {
                    root.clone()
                } else {
                    std::path::PathBuf::from(&config.storage.download_dir)
                }
            } else {
                std::path::PathBuf::from(&config.storage.download_dir)
            }
        } else {
            std::path::PathBuf::from(&config.storage.download_dir)
        };
        let path = base_dir.join(&meta_task.name);
        let _ = self
            .storage_tx
            .send(crate::storage::StorageRequest::RegisterTask {
                task_id: id,
                path,
                total_length: meta_task.total_length,
                checksum: meta_task.checksum.clone(),
                padding_ranges: Vec::new(),
            })
            .await;

        let is_blocked = {
            let mut blocked = false;
            for &parent_id in &depends_on {
                if let Some(parent) = self.tasks.get(&parent_id) {
                    if parent.phase != crate::task::DownloadPhase::Complete {
                        blocked = true;
                        break;
                    }
                } else {
                    blocked = true;
                    break;
                }
            }
            blocked
        };

        if is_blocked {
            meta_task.phase = crate::task::DownloadPhase::Waiting;
        }

        self.tasks.insert(id, meta_task);

        if !is_blocked {
            self.preempt_resources_for_high_priority(id).await;
            self.start_task_loops_with_bitfield(id, token, loaded_bitfield)
                .await?;
        }

        let _ = self.event_tx.send(Event::TaskAdded(id));
        Ok(())
    }
}
