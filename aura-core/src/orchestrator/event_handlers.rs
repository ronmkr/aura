use super::{Event, Orchestrator};
use crate::orchestrator::command::AddTaskArgs;
use crate::task::DownloadPhase;
use crate::{Result, TaskId};
use tracing::{error, info};

impl Orchestrator {
    pub(crate) async fn handle_storage_event(
        &mut self,
        event: crate::storage::StorageEvent,
    ) -> Result<()> {
        match event {
            crate::storage::StorageEvent::Completed(id) => {
                info!(%id, "Storage reported completion");
                if let Some(task) = self.tasks.get_mut(&id) {
                    task.phase = DownloadPhase::Complete;
                    if task.seeding_start_time.is_none() {
                        task.seeding_start_time = Some(chrono::Utc::now());
                    }

                    let duration_secs = task
                        .created_at
                        .map(|t| (chrono::Utc::now() - t).num_seconds().max(0) as u64)
                        .unwrap_or(0);
                    let record = crate::history::CompletedTaskRecord {
                        id: task.id.0.to_string(),
                        name: task.name.clone(),
                        uris: task.subtasks.iter().map(|s| s.uri.clone()).collect(),
                        total_bytes: task.total_length,
                        downloaded_bytes: task.completed_length,
                        uploaded_bytes: task.uploaded_length,
                        duration_secs,
                        checksum_verified: Some(true),
                        phase: "Complete".to_string(),
                        error: None,
                        completed_at: chrono::Utc::now(),
                    };
                    let config = self.config.load();
                    crate::history::HistoryManager::append_record(&config, record);
                    self.notification_service.notify_complete(&task.name);
                    let _ = self.event_tx.send(Event::TaskProgress {
                        id,
                        completed_bytes: task.total_length,
                        uploaded_bytes: task.uploaded_length,
                        total_bytes: task.total_length,
                    });

                    let (follow_on, tenant_id, priority, streaming_mode, task_name) =
                        if let Some(task) = self.tasks.get(&id) {
                            (
                                task.follow_on.clone(),
                                task.tenant_id.clone(),
                                task.priority,
                                task.streaming_mode,
                                task.name.clone(),
                            )
                        } else {
                            (None, None, 0, false, String::new())
                        };

                    // Handle follow-on actions (ADR 0029)
                    if let Some(follow_on) = follow_on {
                        let config = self.config.load();
                        let base_dir = if let Some(ref tid) = tenant_id {
                            if let Some(ctx) = self.tenants.get(tid) {
                                ctx.disk_path_root.clone().unwrap_or_else(|| {
                                    std::path::PathBuf::from(&config.storage.download_dir)
                                })
                            } else {
                                std::path::PathBuf::from(&config.storage.download_dir)
                            }
                        } else {
                            std::path::PathBuf::from(&config.storage.download_dir)
                        };
                        let file_path = base_dir.join(&task_name);

                        match follow_on {
                            crate::task::FollowOnAction::AutoStartTorrent => {
                                if file_path
                                    .extension()
                                    .map(|e| e == "torrent")
                                    .unwrap_or(false)
                                {
                                    info!(%id, ?file_path, "Auto-starting follow-on Torrent task");
                                    let new_id = TaskId(rand::random());
                                    let _ = self
                                        .handle_add_task(AddTaskArgs {
                                            id: new_id,
                                            tenant_id: tenant_id.clone(),
                                            name: "unnamed".to_string(),
                                            sources: vec![(
                                                file_path.to_string_lossy().to_string(),
                                                crate::task::TaskType::BitTorrent,
                                            )],
                                            checksum: None,
                                            priority,
                                            streaming_mode,
                                            depends_on: Vec::new(),
                                            follow_on: None,
                                        })
                                        .await;
                                }
                            }
                            crate::task::FollowOnAction::AutoStartMetalink => {
                                if file_path
                                    .extension()
                                    .map(|e| e == "metalink" || e == "meta4")
                                    .unwrap_or(false)
                                {
                                    info!(%id, ?file_path, "Auto-starting follow-on Metalink task");
                                    let new_id = TaskId(rand::random());
                                    let _ = self
                                        .handle_add_task(AddTaskArgs {
                                            id: new_id,
                                            tenant_id: tenant_id.clone(),
                                            name: "unnamed".to_string(),
                                            sources: vec![(
                                                file_path.to_string_lossy().to_string(),
                                                crate::task::TaskType::Http,
                                            )], // Task type doesn't strictly matter for metalink entry point
                                            checksum: None,
                                            priority,
                                            streaming_mode,
                                            depends_on: Vec::new(),
                                            follow_on: None,
                                        })
                                        .await;
                                }
                            }
                            crate::task::FollowOnAction::Custom(uri) => {
                                info!(%id, %uri, "Starting custom follow-on task");
                                let new_id = TaskId(rand::random());
                                let _ = self
                                    .handle_add_task(AddTaskArgs {
                                        id: new_id,
                                        tenant_id: tenant_id.clone(),
                                        name: "unnamed".to_string(),
                                        sources: vec![(uri, crate::task::TaskType::Http)],
                                        checksum: None,
                                        priority,
                                        streaming_mode,
                                        depends_on: Vec::new(),
                                        follow_on: None,
                                    })
                                    .await;
                            }
                        }
                    }
                }
                let _ = self.event_tx.send(Event::TaskCompleted(id));
                self.check_waiting_tasks().await;
            }
            crate::storage::StorageEvent::Error(id, err) => {
                error!(%id, %err, "Storage reported fatal error; pausing task");
                let mut exists = false;
                if let Some(task) = self.tasks.get_mut(&id) {
                    task.phase = DownloadPhase::Error;
                    exists = true;

                    let duration_secs = task
                        .created_at
                        .map(|t| (chrono::Utc::now() - t).num_seconds().max(0) as u64)
                        .unwrap_or(0);
                    let record = crate::history::CompletedTaskRecord {
                        id: task.id.0.to_string(),
                        name: task.name.clone(),
                        uris: task.subtasks.iter().map(|s| s.uri.clone()).collect(),
                        total_bytes: task.total_length,
                        downloaded_bytes: task.completed_length,
                        uploaded_bytes: task.uploaded_length,
                        duration_secs,
                        checksum_verified: Some(false),
                        phase: "Error".to_string(),
                        error: Some(err.to_string()),
                        completed_at: chrono::Utc::now(),
                    };
                    let config = self.config.load();
                    crate::history::HistoryManager::append_record(&config, record);
                }

                if exists {
                    // Trigger pause logic to cleanup workers
                    let _ = self.handle_pause(id).await;

                    let _ = self.event_tx.send(Event::TaskError {
                        id,
                        message: format!("Storage Error: {}", err),
                    });
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "event_handlers_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "dag_cycle_tests.rs"]
mod dag_cycle_tests;
