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

                let mut uploaded_length = 0;
                if let Some(bt) = self.get_bt_task(id) {
                    uploaded_length = bt
                        .state
                        .uploaded_length
                        .load(std::sync::atomic::Ordering::Relaxed);
                    let mut start_time = bt.state.seeding_start_time.lock().unwrap();
                    if start_time.is_none() {
                        *start_time = Some(chrono::Utc::now());
                    }
                }

                if let Some(task) = self.tasks.get_mut(&id) {
                    task.phase = DownloadPhase::Complete;

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
                        uploaded_bytes: uploaded_length,
                        duration_secs,
                        checksum_verified: Some(true),
                        phase: "Complete".to_string(),
                        error: None,
                        completed_at: chrono::Utc::now(),
                    };
                    let config = self.config.load();
                    crate::history::HistoryManager::append_record(&config, record);
                    self.notification_service.notify_complete(&task.name);
                    self.emit_progress(id);

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
                        let base_dir = self.resolve_base_dir(&tenant_id);
                        let file_path = base_dir.join(&task_name);

                        match follow_on {
                            crate::task::FollowOnAction::AutoStartTorrent => {
                                if file_path
                                    .extension()
                                    .map(|e| e == "torrent")
                                    .unwrap_or(false)
                                {
                                    info!(%id, ?file_path, "Auto-starting follow-on Torrent task");
                                    let new_id = TaskId::random();
                                    let _ = self
                                        .handle_add_task(AddTaskArgs {
                                            id: new_id,
                                            tenant_id: tenant_id.clone(),
                                            name: crate::DEFAULT_TASK_NAME.to_string(),
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
                                    let new_id = TaskId::random();
                                    let _ = self
                                        .handle_add_task(AddTaskArgs {
                                            id: new_id,
                                            tenant_id: tenant_id.clone(),
                                            name: crate::DEFAULT_TASK_NAME.to_string(),
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
                                let new_id = TaskId::random();
                                let _ = self
                                    .handle_add_task(AddTaskArgs {
                                        id: new_id,
                                        tenant_id: tenant_id.clone(),
                                        name: crate::DEFAULT_TASK_NAME.to_string(),
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

                let mut uploaded_length = 0;
                if let Some(bt) = self.get_bt_task(id) {
                    uploaded_length = bt
                        .state
                        .uploaded_length
                        .load(std::sync::atomic::Ordering::Relaxed);
                }

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
                        uploaded_bytes: uploaded_length,
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
