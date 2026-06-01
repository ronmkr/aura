use super::utils::get_part_path;
use super::StorageEngine;
use crate::{Error, Result, TaskId};
use std::path::PathBuf;
use tokio::fs::File;
use tracing::{error, info};

impl StorageEngine {
    pub async fn register_task(
        &mut self,
        id: TaskId,
        path: PathBuf,
        _total_length: u64,
        checksum: Option<crate::Checksum>,
        padding_ranges: Vec<crate::task::Range>,
    ) {
        if let Some(old_path) = self.task_paths.get(&id) {
            if *old_path != path {
                info!(%id, ?old_path, ?path, "Task path updated; moving existing data");
                let old_part = match get_part_path(old_path) {
                    Ok(p) => p,
                    Err(_) => return,
                };
                let new_part = match get_part_path(&path) {
                    Ok(p) => p,
                    Err(_) => return,
                };

                // Close handle if open
                self.handles.pop(&id);

                if old_part.exists() {
                    if let Err(e) = tokio::fs::rename(&old_part, &new_part).await {
                        error!(%id, error = %e, "Failed to move .part file during re-registration");
                    }
                }
            }
        }

        self.task_paths.insert(id, path);
        if let Some(c) = checksum {
            self.task_checksums.insert(id, c);
        }
        self.task_padding_ranges.insert(id, padding_ranges);
        self.next_offsets.entry(id).or_insert(0);
        self.pending_writes.entry(id).or_default();
        self.dirty_buffers.entry(id).or_default();
        self.dirty_sizes.entry(id).or_insert(0);
    }

    pub(crate) async fn preallocate_task(&mut self, id: TaskId, length: u64) -> Result<()> {
        if length == 0 {
            return Ok(());
        }
        let file: &mut File = self.get_or_open_part_file(id).await?;

        let file_clone = file.try_clone().await?.into_std().await;
        let length_clone = length;
        let _ = tokio::task::spawn_blocking(move || {
            let _ = crate::storage::sys::harden_file(&file_clone, length_clone);
        })
        .await;

        file.set_len(length).await.map_err(Error::from)
    }
}
