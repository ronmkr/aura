use crate::orchestrator::{Event, Orchestrator};
use crate::task::MetaTask;
use crate::{Result, TaskId};
use tracing::info;

impl Orchestrator {
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
}
