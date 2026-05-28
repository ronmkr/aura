use super::{Event, Orchestrator};
use crate::task::DownloadPhase;
use crate::{Result, TaskId};
use tracing::{debug, error, info};

impl Orchestrator {
    pub(crate) async fn handle_bt_metadata_received(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        torrent: crate::torrent::Torrent,
    ) -> Result<()> {
        if let Some(bt_task) = self.get_bt_task(sub_id) {
            bt_task.state.mature(torrent.clone()).await;

            let metadata = crate::worker::Metadata {
                final_uri: format!("magnet:?xt={}", bt_task.state.info_hash.to_magnet_urn()),
                total_length: Some(torrent.total_length()),
                name: Some(torrent.info.name.clone()),
                range_supported: true,
            };
            self.handle_subtask_matured(meta_id, sub_id, metadata)
                .await?;
        }
        Ok(())
    }

    pub(crate) async fn handle_subtask_matured(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        metadata: crate::worker::Metadata,
    ) -> Result<()> {
        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
            let mut needs_reregister = false;

            if meta_task.total_length == 0 {
                if let Some(len) = metadata.total_length {
                    info!(%meta_id, %len, "Metadata matured: task initialized");
                    meta_task.total_length = len;
                    if metadata.range_supported {
                        meta_task.generate_ranges(128); // Default 128 segments to allow high concurrency
                    } else {
                        info!(%meta_id, "Server does not support Range requests. Falling back to single-stream download.");
                        meta_task
                            .pending_ranges
                            .push(crate::task::Range { start: 0, end: len });
                    }
                    needs_reregister = true;
                } else {
                    info!(%meta_id, "Metadata matured but total length is unknown. Falling back to single-stream download.");
                    meta_task.total_length = 0;
                    meta_task.pending_ranges.push(crate::task::Range {
                        start: 0,
                        end: u64::MAX,
                    });
                    needs_reregister = true;
                }
            }

            // Update name if currently unnamed
            if (meta_task.name == "unnamed" || meta_task.name.is_empty()) && metadata.name.is_some()
            {
                let new_name = metadata.name.clone().unwrap();
                info!(%meta_id, %new_name, "Updating task name from metadata");
                meta_task.name = new_name;
                needs_reregister = true;
            }

            if needs_reregister {
                // Update storage engine
                let download_dir = {
                    let config = self.config.load();
                    config.storage.download_dir.clone()
                };
                let path = std::path::Path::new(&download_dir).join(&meta_task.name);
                let _ = self
                    .storage_tx
                    .send(crate::storage::StorageRequest::RegisterTask {
                        task_id: meta_id,
                        path,
                        total_length: meta_task.total_length,
                        checksum: meta_task.checksum.clone(),
                    })
                    .await;
            }

            if let Some(sub_task) = meta_task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                sub_task.phase = DownloadPhase::Downloading;
            }
        }

        let (should_notify, final_uri, total_length, name) =
            if let Some(meta_task) = self.tasks.get(&meta_id) {
                (
                    meta_task.total_length > 0,
                    metadata.final_uri,
                    meta_task.total_length,
                    metadata.name,
                )
            } else {
                (false, String::new(), 0, None)
            };

        if should_notify {
            let _ = self.event_tx.send(Event::MetadataResolved {
                id: meta_id,
                final_uri,
                total_length,
                name,
            });
        }

        self.dispatch_next_ranges(meta_id, sub_id).await
    }

    pub(crate) async fn handle_range_finished(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        range: crate::task::Range,
    ) -> Result<()> {
        let mut completed = false;
        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
            // Racing coordination: check if other subtasks were also working on this range
            let racing_sub_ids: Vec<TaskId> = meta_task
                .in_flight_ranges
                .iter()
                .filter(|(sid, r)| *r == range && *sid != sub_id)
                .map(|(sid, _)| *sid)
                .collect();

            if !racing_sub_ids.is_empty() {
                debug!(%meta_id, ?range, racing = racing_sub_ids.len(), "Range finished; canceling racing workers");
                for racing_sid in racing_sub_ids {
                    // For non-BT tasks, we rely on the in_flight_ranges cleanup and next loop check.
                    if let Some(sub) = meta_task.subtasks.iter_mut().find(|s| s.id == racing_sid) {
                        sub.assigned_ranges.retain(|r| *r != range);
                    }
                    if let Some(w_token) = self.worker_cancellation_tokens.remove(&racing_sid) {
                        w_token.cancel();
                    }
                }
            }

            meta_task.mark_range_complete(sub_id, range);

            if meta_task.is_complete()
                && meta_task.phase != DownloadPhase::Verifying
                && meta_task.phase != DownloadPhase::Complete
            {
                if meta_task.checksum.is_some() {
                    info!(%meta_id, "All ranges complete for MetaTask, entering Verifying phase");
                    meta_task.phase = DownloadPhase::Verifying;
                } else {
                    info!(%meta_id, "All ranges complete for MetaTask, entering seeding phase");
                    meta_task.phase = DownloadPhase::Complete;
                    if meta_task.seeding_start_time.is_none() {
                        meta_task.seeding_start_time = Some(chrono::Utc::now());
                    }
                }
                completed = true;
            }
        }

        if completed {
            let _ = self
                .storage_tx
                .send(crate::storage::StorageRequest::Complete(meta_id))
                .await;
        }

        self.dispatch_next_ranges(meta_id, sub_id).await
    }

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
                    let _ = self.event_tx.send(Event::TaskProgress {
                        id,
                        completed_bytes: task.total_length,
                        uploaded_bytes: task.uploaded_length,
                        total_bytes: task.total_length,
                    });
                }
                let event = Event::TaskCompleted(id);
                let _ = self.event_tx.send(event.clone());
                self.hook_manager.handle_event(&event).await;
            }
            crate::storage::StorageEvent::Error(id, err) => {
                error!(%id, %err, "Storage reported fatal error; pausing task");
                let mut exists = false;
                if let Some(task) = self.tasks.get_mut(&id) {
                    task.phase = DownloadPhase::Error;
                    exists = true;
                }

                if exists {
                    // Trigger pause logic to cleanup workers
                    let _ = self.handle_pause(id).await;

                    let event = Event::TaskError {
                        id,
                        message: format!("Storage Error: {}", err),
                    };
                    let _ = self.event_tx.send(event.clone());
                    self.hook_manager.handle_event(&event).await;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{DownloadPhase, MetaTask, Range, SubTask, TaskType};
    use crate::TaskId;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn make_test_orchestrator() -> (
        Orchestrator,
        mpsc::Receiver<crate::storage::StorageRequest>,
        tempfile::TempDir,
    ) {
        let (_command_tx, command_rx) = mpsc::channel(100);
        let (storage_tx, storage_rx) = mpsc::channel(100);
        let (_completion_tx, completion_rx) = mpsc::channel(100);
        let (dht_tx, _) = mpsc::channel(100);
        let (lpd_tx, _) = mpsc::channel(100);
        let (nat_tx, _) = mpsc::channel(100);

        let config = crate::Config::default();
        let config_swap = Arc::new(arc_swap::ArcSwap::from_pointee(config));
        let temp_dir = tempfile::tempdir().unwrap();
        let db = sled::open(temp_dir.path()).unwrap();
        let dns_resolver = Arc::new(
            hickory_resolver::TokioResolver::builder_tokio()
                .unwrap()
                .build()
                .unwrap(),
        );

        let (orchestrator, _event_tx) = Orchestrator::new(
            command_rx,
            storage_tx,
            completion_rx,
            dht_tx,
            lpd_tx,
            nat_tx,
            config_swap,
            db,
            dns_resolver,
        );

        (orchestrator, storage_rx, temp_dir)
    }

    #[tokio::test]
    async fn test_racing_workers_are_cancelled_on_range_finished() {
        let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

        let meta_id = TaskId(1);
        let sub1_id = TaskId(101);
        let sub2_id = TaskId(102);
        let range = Range {
            start: 0,
            end: 1000,
        };

        let sub1 = SubTask {
            id: sub1_id,
            task_type: TaskType::Http,
            uri: "http://example.com/sub1".to_string(),
            assigned_ranges: vec![range],
            total_length: 1000,
            completed_length: 0,
            active: true,
            phase: DownloadPhase::Downloading,
            target_concurrency: 1,
            recent_bytes_downloaded: 0,
            ewma_throughput: 0.0,
            retry_count: 0,
        };

        let sub2 = SubTask {
            id: sub2_id,
            task_type: TaskType::Http,
            uri: "http://example.com/sub2".to_string(),
            assigned_ranges: vec![range],
            total_length: 1000,
            completed_length: 0,
            active: true,
            phase: DownloadPhase::Downloading,
            target_concurrency: 1,
            recent_bytes_downloaded: 0,
            ewma_throughput: 0.0,
            retry_count: 0,
        };

        let meta = MetaTask {
            id: meta_id,
            name: "test".to_string(),
            total_length: 1000,
            completed_length: 0,
            uploaded_length: 0,
            phase: DownloadPhase::Downloading,
            priority: 100,
            streaming_mode: false,
            subtasks: vec![sub1, sub2],
            pending_ranges: vec![],
            in_flight_ranges: vec![(sub1_id, range), (sub2_id, range)],
            checksum: None,
            seeding_start_time: None,
            blacklisted_uris: vec![],
            extensions: HashMap::new(),
        };

        orch.tasks.insert(meta_id, meta);

        // Register worker tokens in Orchestrator
        let token_sub1 = CancellationToken::new();
        let token_sub2 = CancellationToken::new();
        orch.worker_cancellation_tokens
            .insert(sub1_id, token_sub1.clone());
        orch.worker_cancellation_tokens
            .insert(sub2_id, token_sub2.clone());

        // Call handle_range_finished for sub1.
        // This finishes the range. Since sub2 was racing for the same range, it should be cancelled!
        let res = orch.handle_range_finished(meta_id, sub1_id, range).await;
        assert!(res.is_ok());

        // Verify that sub2's token was cancelled!
        assert!(
            token_sub2.is_cancelled(),
            "Racing worker sub2 should be cancelled"
        );
        // Verify that sub1's token is NOT cancelled (sub1 finished successfully)
        assert!(
            !token_sub1.is_cancelled(),
            "Finished worker sub1 should not be cancelled"
        );

        // Verify that sub2 was removed from worker_cancellation_tokens
        assert!(!orch.worker_cancellation_tokens.contains_key(&sub2_id));
    }
}
