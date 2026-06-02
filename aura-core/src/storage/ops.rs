use super::utils::get_part_path;
use super::StorageEngine;
use crate::worker::Segment;
use crate::Error;
use crate::{Result, TaskId};
use bytes::{Bytes, BytesMut};
use std::sync::Arc;
use tokio::fs::{self, File, OpenOptions};
use tracing::debug;

impl StorageEngine {
    pub(crate) async fn handle_read(&mut self, id: TaskId, segment: Segment) -> Result<Bytes> {
        // Flush any in-memory dirty buffers immediately to ensure read-after-write consistency
        self.flush_dirty_buffer_immediate(id).await?;

        let file: &mut File = self.get_or_open_part_file(id).await?;
        use tokio::io::AsyncReadExt;
        use tokio::io::AsyncSeekExt;

        let file_len = file.metadata().await?.len();
        let read_len = std::cmp::min(segment.length, file_len.saturating_sub(segment.offset));

        file.seek(std::io::SeekFrom::Start(segment.offset)).await?;

        let mut buf = vec![0u8; segment.length as usize];
        if read_len > 0 {
            file.read_exact(&mut buf[..read_len as usize]).await?;
        }

        // Explicitly zero out padding ranges in the buffer
        let padding_ranges = self
            .task_padding_ranges
            .get(&id)
            .cloned()
            .unwrap_or_default();
        let sub_pads = super::engine::get_padding_subranges_impl(
            &padding_ranges,
            segment.offset,
            segment.length,
        );

        for pad in sub_pads {
            let start_in_buf = (pad.start - segment.offset) as usize;
            let end_in_buf = (pad.end - segment.offset) as usize;
            buf[start_in_buf..end_in_buf].fill(0);
        }

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

            let padding_ranges = self
                .task_padding_ranges
                .get(&id)
                .cloned()
                .unwrap_or_default();

            // Filter padding before pushing to dirty buffer
            let subranges =
                super::engine::get_non_padding_subranges_impl(&padding_ranges, segment.offset, len);
            for sub in subranges {
                let sub_offset = sub.start;
                let sub_len = sub.length();

                // Check for discontinuity in dirty buffer (caused by padding skip)
                let needs_flush = if let Some(dirty) = self.dirty_buffers.get(&id) {
                    if let Some(last) = dirty.last() {
                        sub_offset != (last.0 + last.1.len() as u64)
                    } else {
                        false
                    }
                } else {
                    false
                };

                if needs_flush {
                    self.flush_dirty_buffer(id).await?;
                }

                let start_in_data = (sub_offset - segment.offset) as usize;
                let end_in_data = start_in_data + sub_len as usize;
                let sub_data = BytesMut::from(&data[start_in_data..end_in_data]);

                if let Some(dirty) = self.dirty_buffers.get_mut(&id) {
                    dirty.push((sub_offset, sub_data));
                }
                if let Some(size) = self.dirty_sizes.get_mut(&id) {
                    *size += sub_len as usize;
                }
            }

            let mut current_offset = next_offset + len;

            let mut to_flush = Vec::new();
            if let Some(pending) = self.pending_writes.get_mut(&id) {
                while let Some(p_data) = pending.remove(&current_offset) {
                    let p_len = p_data.len() as u64;

                    // Filter padding for pending writes as well
                    let p_subranges = super::engine::get_non_padding_subranges_impl(
                        &padding_ranges,
                        current_offset,
                        p_len,
                    );
                    for sub in p_subranges {
                        let start_in_p = (sub.start - current_offset) as usize;
                        let end_in_p = start_in_p + sub.length() as usize;
                        to_flush.push((sub.start, BytesMut::from(&p_data[start_in_p..end_in_p])));
                    }

                    current_offset += p_len;
                }
            }

            for (offset, p_data) in to_flush {
                let p_len = p_data.len();

                // Check for discontinuity again
                let needs_flush = if let Some(dirty) = self.dirty_buffers.get(&id) {
                    if let Some(last) = dirty.last() {
                        offset != (last.0 + last.1.len() as u64)
                    } else {
                        false
                    }
                } else {
                    false
                };

                if needs_flush {
                    self.flush_dirty_buffer(id).await?;
                }

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

    pub(crate) async fn flush_dirty_buffer_immediate(&mut self, id: TaskId) -> Result<()> {
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

        let file = self.get_or_open_part_file(id).await?;
        use tokio::io::AsyncSeekExt;
        use tokio::io::AsyncWriteExt;

        file.seek(std::io::SeekFrom::Start(offset)).await?;
        let mut total_len = 0;
        for d in &data {
            file.write_all(d).await?;
            total_len += d.len() as u64;
        }

        crate::storage::sys::apply_fadvise_dontneed(file, offset, total_len);

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
        let file: &mut File = self.get_or_open_part_file(task.task_id).await?;
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
