use super::StorageEngine;
use crate::worker::Segment;
use crate::Error;
use crate::{Result, TaskId};
use bytes::{Bytes, BytesMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::{self, File, OpenOptions};
use tracing::{debug, info};

impl StorageEngine {
    pub(crate) async fn handle_read(&mut self, id: TaskId, segment: Segment) -> Result<Bytes> {
        let buffer_size = self
            .config
            .as_ref()
            .map(|cfg: &Arc<arc_swap::ArcSwap<crate::Config>>| {
                cfg.load().storage.read_ahead_kb as usize * 1024
            })
            .unwrap_or(128 * 1024); // Default to 128 KB seeding read buffer capacity

        let file = self.get_or_open_part_file(id).await?;
        use tokio::io::AsyncReadExt;
        use tokio::io::AsyncSeekExt;
        file.seek(std::io::SeekFrom::Start(segment.offset)).await?;

        let mut reader = tokio::io::BufReader::with_capacity(buffer_size, file);
        let mut buf = vec![0u8; segment.length as usize];
        reader.read_exact(&mut buf).await?;
        Ok(Bytes::from(buf))
    }

    pub(crate) async fn handle_write(
        &mut self,
        id: TaskId,
        segment: Segment,
        data: BytesMut,
    ) -> Result<()> {
        let next_offset = *self.next_offsets.get(&id).unwrap_or(&0);

        if segment.offset == next_offset {
            let len = data.len() as u64;

            // Push to dirty buffer instead of immediate write
            if let Some(dirty) = self.dirty_buffers.get_mut(&id) {
                dirty.push((segment.offset, data));
            }
            if let Some(size) = self.dirty_sizes.get_mut(&id) {
                *size += len as usize;
            }

            let mut current_offset = next_offset + len;

            let mut to_flush = Vec::new();
            if let Some(pending) = self.pending_writes.get_mut(&id) {
                while let Some(p_data) = pending.remove(&current_offset) {
                    let p_len = p_data.len() as u64;
                    to_flush.push((current_offset, p_data));
                    current_offset += p_len;
                }
            }

            for (offset, p_data) in to_flush {
                let p_len = p_data.len();
                if let Some(dirty) = self.dirty_buffers.get_mut(&id) {
                    dirty.push((offset, p_data));
                }
                if let Some(size) = self.dirty_sizes.get_mut(&id) {
                    *size += p_len;
                }
            }

            self.next_offsets.insert(id, current_offset);

            // Flush if we hit the write buffer threshold
            let threshold = self
                .config
                .as_ref()
                .map(|cfg: &Arc<arc_swap::ArcSwap<crate::Config>>| {
                    cfg.load().storage.write_buffer_kb as usize * 1024
                })
                .unwrap_or(4_194_304); // Default to 4MB if no config provided

            if let Some(&size) = self.dirty_sizes.get(&id) {
                if size >= threshold {
                    self.flush_dirty_buffer(id).await?;
                }
            }
        } else {
            debug!(%id, offset = %segment.offset, expected = %next_offset, "Buffering out-of-order piece");
            if let Some(pending) = self.pending_writes.get_mut(&id) {
                pending.insert(segment.offset, data);
            }
        }
        Ok(())
    }

    pub(crate) async fn flush_dirty_buffer(&mut self, id: TaskId) -> Result<()> {
        let buffers = if let Some(dirty) = self.dirty_buffers.get_mut(&id) {
            std::mem::take(dirty)
        } else {
            Vec::new()
        };

        if buffers.is_empty() {
            return Ok(());
        }

        let offset = buffers.first().unwrap().0;
        let data = buffers.into_iter().map(|(_, d)| d).collect::<Vec<_>>();

        use super::scheduler::{IoPriority, IoTask};
        use tokio::time::{Duration, Instant};

        self.scheduler.enqueue(IoTask {
            task_id: id,
            offset,
            data,
            deadline: Instant::now() + Duration::from_millis(500),
            priority: IoPriority::Normal,
        });

        if let Some(size) = self.dirty_sizes.get_mut(&id) {
            *size = 0;
        }

        Ok(())
    }

    pub(crate) async fn flush_all_pending(&mut self, id: TaskId) -> Result<()> {
        self.flush_dirty_buffer(id).await?;

        if let Some(pending) = self.pending_writes.remove(&id) {
            use super::scheduler::{IoPriority, IoTask};
            use tokio::time::{Duration, Instant};
            for (offset, data) in pending {
                self.scheduler.enqueue(IoTask {
                    task_id: id,
                    offset,
                    data: vec![data],
                    deadline: Instant::now() + Duration::from_millis(100),
                    priority: IoPriority::High,
                });
            }
        }
        Ok(())
    }

    pub(crate) async fn execute_io_task(&mut self, task: super::scheduler::IoTask) -> Result<()> {
        let file = self.get_or_open_part_file(task.task_id).await?;
        use tokio::io::AsyncSeekExt;
        use tokio::io::AsyncWriteExt;

        file.seek(std::io::SeekFrom::Start(task.offset)).await?;
        let mut total_len = 0;
        for data in &task.data {
            file.write_all(data).await?;
            total_len += data.len() as u64;
        }

        crate::storage::sys::apply_fadvise_dontneed(file, task.offset, total_len);

        Ok(())
    }

    pub(crate) async fn handle_complete(&mut self, id: TaskId) -> Result<()> {
        let base_path = self.task_paths.get(&id).ok_or(Error::TaskNotFound(id))?;
        let part_path = get_part_path(base_path)?;
        let hardened_base = crate::storage::sys::harden_path(base_path);

        self.flush_all_pending(id).await?;

        let tasks = self.scheduler.extract_all_for_task(id);
        for task in tasks {
            self.execute_io_task(task).await?;
        }

        // Close the write handle before verification to ensure all data is committed
        // and to allow clean read-only access.
        self.handles.pop(&id);

        // fsync the .part file to ensure all data is on disk before exposing it under the final name
        let file = fs::OpenOptions::new().read(true).open(&part_path).await?;
        file.sync_all().await?;

        info!(%id, from = ?part_path, to = ?hardened_base, "Performing atomic completion rename");
        fs::rename(&part_path, &hardened_base).await?;

        // Sync parent directory to ensure metadata rename is durable on Unix
        sync_parent_dir(&hardened_base).await;

        let _ = self
            .completion_tx
            .send(crate::storage::StorageEvent::Completed(id))
            .await;

        Ok(())
    }

    pub(crate) async fn verify_checksum(&mut self, id: TaskId) -> Result<()> {
        let checksum = match self.task_checksums.get(&id) {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        info!(%id, ?checksum, "Verifying file integrity");

        let base_path = self.task_paths.get(&id).ok_or(Error::TaskNotFound(id))?;
        let part_path = get_part_path(base_path)?;

        let file = File::open(&part_path).await?;
        let mut reader = tokio::io::BufReader::new(file);

        use md5::Digest;
        use tokio::io::AsyncReadExt;

        let actual = match checksum {
            crate::Checksum::Md5(ref expected) => {
                let mut hasher = md5::Md5::default();
                let mut buffer = [0u8; 65536];
                loop {
                    let n = reader.read(&mut buffer).await?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
            crate::Checksum::Sha1(ref expected) => {
                let mut hasher = sha1::Sha1::default();
                let mut buffer = [0u8; 65536];
                loop {
                    let n = reader.read(&mut buffer).await?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
            crate::Checksum::Sha256(ref expected) => {
                let mut hasher = sha2::Sha256::default();
                let mut buffer = [0u8; 65536];
                loop {
                    let n = reader.read(&mut buffer).await?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
            crate::Checksum::Sha512(ref expected) => {
                let mut hasher = sha2::Sha512::default();
                let mut buffer = [0u8; 65536];
                loop {
                    let n = reader.read(&mut buffer).await?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
        };

        let (expected, actual_hash) = actual;

        if expected.to_lowercase() != actual_hash.to_lowercase() {
            return Err(Error::Storage(format!(
                "Checksum mismatch: expected {}, got {}",
                expected, actual_hash
            )));
        }

        info!(%id, "Integrity verification successful");
        Ok(())
    }

    pub(crate) async fn get_or_open_part_file(&mut self, id: TaskId) -> Result<&mut File> {
        if !self.handles.contains(&id) {
            let base_path = self.task_paths.get(&id).ok_or(Error::TaskNotFound(id))?;
            let part_path = get_part_path(base_path)?;

            if let Some(parent) = part_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            let file = OpenOptions::new()
                .write(true)
                .read(true)
                .create(true)
                .truncate(false)
                .open(&part_path)
                .await?;

            crate::storage::sys::apply_fadvise_sequential(&file);

            if let Some((_, evicted_file)) = self.handles.push(id, file) {
                let _ = evicted_file.sync_all().await;
            }
        }

        Ok(self
            .handles
            .get_mut(&id)
            .expect("File handle must exist after open/insert"))
    }
}

async fn sync_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        let parent_clone = parent.to_path_buf();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(dir) = std::fs::File::open(&parent_clone) {
                let _ = dir.sync_all();
            }
        })
        .await;
    }
}

pub fn get_part_path(base_path: &Path) -> Result<PathBuf> {
    let mut part_path = crate::storage::sys::harden_path(base_path);
    let mut filename = part_path
        .file_name()
        .ok_or_else(|| Error::Task(TaskId(0), "Invalid filename".to_string()))? // Placeholder ID as we don't have it here
        .to_os_string();
    filename.push(".part");
    part_path.set_file_name(filename);
    Ok(part_path)
}
