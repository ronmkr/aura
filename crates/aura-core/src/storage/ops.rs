use super::StorageEngine;
use crate::worker::Segment;
use crate::Error;
use crate::{Result, TaskId};
use bytes::Bytes;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info};

impl StorageEngine {
    pub(crate) async fn handle_read(&mut self, id: TaskId, segment: Segment) -> Result<Bytes> {
        let file = self.get_or_open_part_file(id).await?;
        use tokio::io::AsyncReadExt;
        use tokio::io::AsyncSeekExt;
        file.seek(std::io::SeekFrom::Start(segment.offset)).await?;
        let mut buf = vec![0u8; segment.length as usize];
        file.read_exact(&mut buf).await?;
        Ok(Bytes::from(buf))
    }

    pub(crate) async fn handle_write(
        &mut self,
        id: TaskId,
        segment: Segment,
        data: Bytes,
    ) -> Result<()> {
        let next_offset = *self.next_offsets.get(&id).unwrap_or(&0);

        if segment.offset == next_offset {
            self.write_to_disk(id, segment.offset, data).await?;
            let mut current_offset = next_offset + segment.length;

            let mut to_flush = Vec::new();
            if let Some(pending) = self.pending_writes.get_mut(&id) {
                while let Some(data) = pending.remove(&current_offset) {
                    let len = data.len() as u64;
                    to_flush.push((current_offset, data));
                    current_offset += len;
                }
            }

            for (offset, data) in to_flush {
                self.write_to_disk(id, offset, data).await?;
            }

            self.next_offsets.insert(id, current_offset);
        } else {
            debug!(%id, offset = %segment.offset, expected = %next_offset, "Buffering out-of-order piece");
            if let Some(pending) = self.pending_writes.get_mut(&id) {
                pending.insert(segment.offset, data);
            }
        }
        Ok(())
    }

    pub(crate) async fn write_to_disk(
        &mut self,
        id: TaskId,
        offset: u64,
        data: Bytes,
    ) -> Result<()> {
        let file = self.get_or_open_part_file(id).await?;
        use tokio::io::AsyncSeekExt;
        use tokio::io::AsyncWriteExt;
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        file.write_all(&data).await?;
        Ok(())
    }

    pub(crate) async fn flush_all_pending(&mut self, id: TaskId) -> Result<()> {
        if let Some(pending) = self.pending_writes.remove(&id) {
            for (offset, data) in pending {
                self.write_to_disk(id, offset, data).await?;
            }
        }
        Ok(())
    }

    pub(crate) async fn handle_complete(&mut self, id: TaskId) -> Result<()> {
        self.handles.remove(&id);

        let base_path = self
            .task_paths
            .get(&id)
            .ok_or_else(|| Error::Storage("Task path not registered".to_string()))?;

        let part_path = get_part_path(base_path)?;

        info!(%id, from = ?part_path, to = ?base_path, "Performing atomic completion rename");
        fs::rename(&part_path, base_path).await?;

        let _ = self.completion_tx.send(id).await;

        Ok(())
    }
}

pub(crate) fn get_part_path(base_path: &Path) -> Result<PathBuf> {
    let mut part_path = base_path.to_path_buf();
    let mut filename = part_path
        .file_name()
        .ok_or_else(|| Error::Storage("Invalid filename".to_string()))?
        .to_os_string();
    filename.push(".part");
    part_path.set_file_name(filename);
    Ok(part_path)
}
