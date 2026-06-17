use crate::orchestrator::Orchestrator;
use crate::task::DownloadPhase;
use crate::{Result, TaskId};
use tracing::info;

impl Orchestrator {
    pub(crate) async fn maybe_spawn_recheck(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
    ) -> Result<bool> {
        let (path, part_path, should_proceed) = {
            let meta_task = match self.tasks.get(&meta_id) {
                Some(t) => t,
                None => return Ok(false),
            };

            if meta_task.phase == DownloadPhase::Complete
                || meta_task.phase == DownloadPhase::Verifying
                || meta_task.total_length == 0
            {
                return Ok(false);
            }

            let base_dir = self.resolve_base_dir(&meta_task.tenant_id);
            let path = self.mapping_engine.resolve_path(meta_task, &base_dir);
            let part_path =
                crate::storage::utils::get_part_path(&path).unwrap_or_else(|_| path.clone());
            (path, part_path, true)
        };

        if !should_proceed {
            return Ok(false);
        }

        let exists_target = path.exists();
        let exists_part = part_path.exists();

        if !exists_target && !exists_part {
            return Ok(false);
        }

        if exists_target && !exists_part {
            let _ = tokio::fs::rename(&path, &part_path).await;
        }

        let meta_task = match self.tasks.get_mut(&meta_id) {
            Some(t) => t,
            None => return Ok(false),
        };

        info!(%meta_id, "Triggering background file/piece recheck for resume");
        meta_task.phase = DownloadPhase::Verifying;
        let throttle_ms = self.config.load().storage.recheck_throttle_ms;
        let subtask_tx = self.subtask_tx.clone();

        let is_bt = meta_task
            .subtasks
            .iter()
            .any(|s| s.id == sub_id && s.task_type == crate::task::TaskType::BitTorrent);

        if is_bt {
            if let Some(bt_task) = self.get_bt_task(sub_id) {
                let torrent_guard = bt_task.state.torrent.lock().await;
                if let Some(ref torrent) = *torrent_guard {
                    let torrent_clone = torrent.clone();
                    tokio::spawn(async move {
                        let _ = crate::storage::recheck::recheck_bittorrent(
                            meta_id,
                            sub_id,
                            &part_path,
                            torrent_clone,
                            subtask_tx,
                            throttle_ms,
                        )
                        .await;
                    });
                    return Ok(true);
                }
            }
        } else {
            let checksum = meta_task.checksum.clone();
            let total_length = meta_task.total_length;
            let saved_progress = if total_length > 0 {
                Some(meta_task.completed_length as f64 / total_length as f64)
            } else {
                None
            };
            tokio::spawn(async move {
                let _ = crate::storage::recheck::recheck_non_swarm(
                    meta_id,
                    sub_id,
                    &part_path,
                    total_length,
                    checksum,
                    saved_progress,
                    subtask_tx,
                    throttle_ms,
                )
                .await;
            });
            return Ok(true);
        }

        Ok(false)
    }
}
