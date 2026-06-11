use super::utils::get_part_path;
use super::StorageEngine;
use crate::worker::Segment;
use crate::Error;
use crate::{Result, TaskId};
use bytes::{Bytes, BytesMut};
use std::sync::Arc;
use tokio::fs::{self, File, OpenOptions};

impl StorageEngine {
    pub(crate) async fn handle_read(&mut self, id: TaskId, segment: Segment) -> Result<Bytes> {
        // Flush any in-memory dirty buffers immediately to ensure read-after-write consistency
        self.flush_dirty_buffer_immediate(id).await?;
        self.flush_scheduler_tasks(id).await?;

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
        if data.is_empty() {
            return Ok(());
        }

        let padding_ranges = self
            .task_padding_ranges
            .get(&id)
            .cloned()
            .unwrap_or_default();

        let mut threshold = self
            .config
            .as_ref()
            .map(|cfg: &Arc<arc_swap::ArcSwap<crate::Config>>| {
                cfg.load().storage.write_buffer_kb as usize * 1024
            })
            .unwrap_or(4_194_304);

        if self.locker.is_network_share(&id) {
            threshold *= 4;
        }

        let ready_blocks =
            self.aggregator
                .add_write(id, segment.offset, data, &padding_ranges, threshold);

        let deadline_ms = self
            .config
            .as_ref()
            .map(|c| c.load().storage.io_deadline_ms)
            .unwrap_or(500);

        for block in ready_blocks {
            use super::scheduler::{IoPriority, IoTask};
            use tokio::time::{Duration, Instant};

            self.scheduler.enqueue(IoTask {
                task_id: id,
                offset: block.offset,
                data: block.data,
                deadline: Instant::now() + Duration::from_millis(deadline_ms),
                priority: IoPriority::Normal,
            });
        }

        Ok(())
    }

    pub(crate) async fn flush_dirty_buffer(&mut self, id: TaskId) -> Result<()> {
        let deadline_ms = self
            .config
            .as_ref()
            .map(|c| c.load().storage.io_deadline_ms)
            .unwrap_or(500);

        if let Some(block) = self.aggregator.take_dirty_block(id) {
            use super::scheduler::{IoPriority, IoTask};
            use tokio::time::{Duration, Instant};

            self.scheduler.enqueue(IoTask {
                task_id: id,
                offset: block.offset,
                data: block.data,
                deadline: Instant::now() + Duration::from_millis(deadline_ms),
                priority: IoPriority::Normal,
            });
        }

        Ok(())
    }

    pub(crate) async fn flush_dirty_buffer_immediate(&mut self, id: TaskId) -> Result<()> {
        if let Some(block) = self.aggregator.take_dirty_block(id) {
            let file = self.get_or_open_part_file(id).await?;
            use tokio::io::AsyncSeekExt;
            use tokio::io::AsyncWriteExt;

            file.seek(std::io::SeekFrom::Start(block.offset)).await?;
            let mut total_len = 0;
            for d in &block.data {
                file.write_all(d).await?;
                total_len += d.len() as u64;
            }

            crate::storage::sys::apply_fadvise_dontneed(file, block.offset, total_len);
        }

        Ok(())
    }

    pub(crate) async fn flush_all_pending(&mut self, id: TaskId) -> Result<()> {
        let blocks = self.aggregator.take_all_pending(id);

        let deadline_ms = self
            .config
            .as_ref()
            .map(|c| c.load().storage.io_deadline_ms / 5) // High priority is 5x faster
            .unwrap_or(100);

        use super::scheduler::{IoPriority, IoTask};
        use tokio::time::{Duration, Instant};

        for block in blocks {
            self.scheduler.enqueue(IoTask {
                task_id: id,
                offset: block.offset,
                data: block.data,
                deadline: Instant::now() + Duration::from_millis(deadline_ms),
                priority: IoPriority::High,
            });
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

    /// Platform-agnostic memory-mapped I/O fallback writer for large files.
    #[allow(dead_code)]
    pub(crate) fn execute_io_task_mmap(&mut self, _task: &super::scheduler::IoTask) -> Result<()> {
        // Placeholder skeleton for memory-mapped I/O fallback.
        // Once activated, large files can be mapped into memory and written directly.
        Ok(())
    }

    pub(crate) async fn get_or_open_part_file(&mut self, id: TaskId) -> Result<&mut File> {
        if !self.handles.contains(&id) {
            let base_path = self.task_paths.get(&id).ok_or(Error::TaskNotFound(id))?;
            self.check_path_sandbox(base_path)?;
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

            self.locker.lock_and_detect_network(id, &file)?;

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

    pub(crate) async fn flush_scheduler_tasks(&mut self, id: TaskId) -> Result<()> {
        let tasks = self.scheduler.extract_all_for_task(id);
        for task in tasks {
            self.execute_io_task(task).await?;
        }
        Ok(())
    }
}
