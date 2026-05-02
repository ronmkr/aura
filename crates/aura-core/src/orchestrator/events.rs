use super::{Event, Orchestrator, SubTaskEvent};
use crate::task::DownloadPhase;
use crate::{Result, TaskId};
use tracing::{debug, info};

impl Orchestrator {
    pub(crate) async fn handle_subtask_event(&mut self, event: SubTaskEvent) -> Result<()> {
        match event {
            SubTaskEvent::Matured(meta_id, sub_id, metadata) => {
                self.handle_subtask_matured(meta_id, sub_id, metadata)
                    .await?;
            }
            SubTaskEvent::RangeFinished(meta_id, sub_id, range) => {
                self.handle_range_finished(meta_id, sub_id, range).await?;
            }
            SubTaskEvent::Failed(meta_id, sub_id, err) => {
                info!(%meta_id, %sub_id, %err, "Subtask failed");
            }
            SubTaskEvent::Downloaded(meta_id, bytes) => {
                self.throttler.consume_download(bytes).await;
                if let Some(task) = self.tasks.get_mut(&meta_id) {
                    task.completed_length += bytes;
                    let _ = self.event_tx.send(Event::TaskProgress {
                        id: meta_id,
                        completed_bytes: task.completed_length,
                        total_bytes: task.total_length,
                    });
                }
            }
            SubTaskEvent::PeerBitfield(meta_id, peer_id, bf) => {
                debug!(%meta_id, ?peer_id, count = bf.count_set(), "Peer bitfield received");
            }
            SubTaskEvent::PeerHave(meta_id, peer_id, idx) => {
                debug!(%meta_id, ?peer_id, idx, "Peer reported piece availability");
            }
            SubTaskEvent::BtTaskRegistered(_meta_id, sub_id, info_hash, task) => {
                self.bt_registry.insert(info_hash, task.clone());
                self.bt_tasks.insert(sub_id, task);
            }
            SubTaskEvent::KillSwitch => {
                let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                for id in ids {
                    let _ = self.handle_pause(id).await;
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn handle_subtask_matured(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        metadata: crate::worker::Metadata,
    ) -> Result<()> {
        let mut initialized = false;
        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
            if meta_task.total_length == 0 {
                if let Some(len) = metadata.total_length {
                    info!(%meta_id, %len, "Metadata matured: task initialized");
                    meta_task.total_length = len;
                    meta_task.generate_ranges(16); // Default 16 segments
                    initialized = true;
                }
            }

            if let Some(sub_task) = meta_task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                sub_task.phase = DownloadPhase::Downloading;
            }
        }

        if initialized {
            let meta_task = self.tasks.get(&meta_id).unwrap();
            let _ = self.event_tx.send(Event::MetadataResolved {
                id: meta_id,
                final_uri: metadata.final_uri,
                total_length: meta_task.total_length,
                name: metadata.name,
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
        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
            meta_task.mark_range_complete(sub_id, range);

            if meta_task.is_complete() {
                info!(%meta_id, "All ranges complete for MetaTask");
                meta_task.phase = DownloadPhase::Complete;
            }
        }

        self.dispatch_next_ranges(meta_id, sub_id).await
    }

    pub(crate) async fn handle_storage_completion(&mut self, id: TaskId) -> Result<()> {
        info!(%id, "Storage reported completion");
        if let Some(task) = self.tasks.get(&id) {
            let _ = self.event_tx.send(Event::TaskProgress {
                id,
                completed_bytes: task.total_length,
                total_bytes: task.total_length,
            });
        }
        let _ = self.event_tx.send(Event::TaskCompleted(id));
        Ok(())
    }
}
