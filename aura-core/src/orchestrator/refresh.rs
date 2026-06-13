use crate::orchestrator::state::Orchestrator;
use crate::orchestrator::telemetry::Event;
use crate::task::DownloadPhase;
use crate::Result;
use crate::TaskId;
use tracing::{info, warn};

impl Orchestrator {
    pub(crate) async fn handle_refresh_matured(
        &mut self,
        meta_id: TaskId,
        _sub_id: TaskId,
        metadata: crate::worker::Metadata,
    ) -> Result<()> {
        // 1. Check if the task exists, and copy its tenant_id, priority, checksum, and name
        let (tenant_id, priority, checksum, mut name) = {
            let meta_task = self
                .tasks
                .get(&meta_id)
                .ok_or(crate::Error::TaskNotFound(meta_id))?;
            (
                meta_task.tenant_id.clone(),
                meta_task.priority,
                meta_task.checksum.clone(),
                meta_task.name.clone(),
            )
        };

        if let Some(new_name) = metadata.name.clone() {
            name = new_name;
        }

        info!(%meta_id, "Refresh returned 200 OK: resetting progress and starting clean re-download");

        // 2. Pause first to cancel existing task loops cleanly
        let _ = self.handle_pause(meta_id).await;

        // Determine paths on disk
        let base_dir = self.resolve_base_dir(&tenant_id);

        let (total_length, part_path, final_path) = {
            let meta_task = self
                .tasks
                .get_mut(&meta_id)
                .ok_or(crate::Error::TaskNotFound(meta_id))?;

            // Reset lengths
            meta_task.completed_length = 0;
            meta_task.total_length = metadata.total_length.unwrap_or(0);
            meta_task.range_supported = metadata.range_supported;
            meta_task.etag = metadata.etag.clone();
            meta_task.last_modified = metadata.last_modified.clone();
            meta_task.name = name.clone();

            // Reset subtasks
            for sub in &mut meta_task.subtasks {
                sub.completed_length = 0;
                sub.total_length = metadata.total_length.unwrap_or(0);
                sub.phase = DownloadPhase::Downloading;
                sub.active = true;
            }

            // Regenerate ranges
            meta_task.generate_ranges(128, None);

            let final_path = base_dir.join(&name);
            let part_path = base_dir.join(format!("{}.part", name));

            (meta_task.total_length, part_path, final_path)
        };

        // 4. Delete files on disk to ensure clean re-download (no active borrows on self.tasks)
        if final_path.exists() {
            let _ = std::fs::remove_file(&final_path);
        }
        if part_path.exists() {
            let _ = std::fs::remove_file(&part_path);
        }

        // 5. Re-register task with storage
        let _ = self
            .storage_client
            .register_task(meta_id, final_path, total_length, checksum, Vec::new())
            .await;

        // 6. Re-register task with throttler
        let config = self.config.load();
        let throttler = self.resolve_throttler(&tenant_id);

        throttler
            .register_task(
                meta_id,
                config.bandwidth.per_task_download_limit,
                config.bandwidth.per_task_upload_limit,
                priority,
            )
            .await;

        // 7. Save state and resume/start task loops
        let _ = self.save_task(meta_id).await;

        {
            let meta_task = self
                .tasks
                .get_mut(&meta_id)
                .ok_or(crate::Error::TaskNotFound(meta_id))?;
            meta_task.phase = DownloadPhase::Downloading;
        }

        let token = tokio_util::sync::CancellationToken::new();
        self.cancellation_tokens.insert(meta_id, token.clone());
        self.start_task_loops_with_bitfield(meta_id, token, None)
            .await?;

        Ok(())
    }

    pub(crate) async fn handle_refresh_not_modified(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
    ) -> Result<()> {
        if let Some(task) = self.tasks.get_mut(&meta_id) {
            info!(%meta_id, %sub_id, "Refresh returned 304 Not Modified: file is current");
            task.phase = DownloadPhase::Complete;
            if let Some(bt) = self.get_bt_task(meta_id) {
                let mut start_time = bt.state.seeding_start_time.lock().unwrap();
                if start_time.is_none() {
                    *start_time = Some(chrono::Utc::now());
                }
            }
            let _ = self.save_task(meta_id).await;
            let _ = self.event_tx.send(Event::TaskCompleted(meta_id));
        }
        Ok(())
    }

    pub(crate) async fn handle_refresh_failed(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        err: String,
    ) -> Result<()> {
        warn!(%meta_id, %sub_id, error = %err, "Refresh check failed");
        Ok(())
    }
}
