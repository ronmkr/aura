use tokio::sync::{mpsc, oneshot};
use tokio::fs::{self, OpenOptions, File};
use tokio::io::{AsyncWriteExt, AsyncSeekExt, AsyncReadExt};
use bytes::Bytes;
use tracing::{info, debug, error};
use std::collections::{HashMap, BTreeMap};
use std::path::{PathBuf, Path};
use crate::{Result, TaskId, Error};
use crate::worker::Segment;

/// Requests sent to the Storage Engine.
#[derive(Debug)]
pub enum StorageRequest {
    /// Write data to a specific task's file.
    Write {
        task_id: TaskId,
        segment: Segment,
        data: Bytes,
    },
    /// Read data from a specific task's file.
    Read {
        task_id: TaskId,
        segment: Segment,
        reply_tx: oneshot::Sender<Result<Bytes>>,
    },
    /// Mark a task as complete and perform atomic rename.
    Complete(TaskId),
}

use crate::buffer_pool::BufferPool;

pub struct StorageEngine {
    request_rx: mpsc::Receiver<StorageRequest>,
    completion_tx: mpsc::Sender<TaskId>,
    /// Maps TaskId to the base target path.
    task_paths: HashMap<TaskId, PathBuf>,
    /// Active file handles for .part files.
    handles: HashMap<TaskId, File>,
    /// Buffers for out-of-order pieces per task.
    pending_writes: HashMap<TaskId, BTreeMap<u64, Bytes>>,
    /// Tracks the next expected offset for sequential writes.
    next_offsets: HashMap<TaskId, u64>,
    /// Centralized buffer pool.
    _pool: BufferPool,
}

impl StorageEngine {
    pub fn new(
        request_rx: mpsc::Receiver<StorageRequest>,
        completion_tx: mpsc::Sender<TaskId>,
    ) -> Self {
        Self { 
            request_rx,
            completion_tx,
            task_paths: HashMap::new(),
            handles: HashMap::new(),
            pending_writes: HashMap::new(),
            next_offsets: HashMap::new(),
            _pool: BufferPool::new(1024 * 1024, 10), // 1MB chunks
        }
    }

    pub fn register_task(&mut self, id: TaskId, path: PathBuf) {
        self.task_paths.insert(id, path);
        self.next_offsets.insert(id, 0);
        self.pending_writes.insert(id, BTreeMap::new());
    }

    pub async fn run(mut self) -> Result<()> {
        info!("Storage Engine started");
        
        while let Some(req) = self.request_rx.recv().await {
            match req {
                StorageRequest::Write { task_id, segment, data } => {
                    if let Err(e) = self.handle_write(task_id, segment, data).await {
                        error!(%task_id, error = %e, "Failed to write data");
                    }
                }
                StorageRequest::Read { task_id, segment, reply_tx } => {
                    let res = self.handle_read(task_id, segment).await;
                    let _ = reply_tx.send(res);
                }
                StorageRequest::Complete(task_id) => {
                    // Flush any remaining pending writes before completion
                    if let Err(e) = self.flush_all_pending(task_id).await {
                        error!(%task_id, error = %e, "Failed to flush pending writes on completion");
                    }
                    if let Err(e) = self.handle_complete(task_id).await {
                        error!(%task_id, error = %e, "Failed to complete task");
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn handle_read(&mut self, id: TaskId, segment: Segment) -> Result<Bytes> {
        let file = self.get_or_open_part_file(id).await?;
        file.seek(std::io::SeekFrom::Start(segment.offset)).await?;
        let mut buf = vec![0u8; segment.length as usize];
        file.read_exact(&mut buf).await?;
        Ok(Bytes::from(buf))
    }

    async fn handle_write(&mut self, id: TaskId, segment: Segment, data: Bytes) -> Result<()> {
        let next_offset = *self.next_offsets.get(&id).unwrap_or(&0);
        
        if segment.offset == next_offset {
            // 1. Write the current piece
            self.write_to_disk(id, segment.offset, data).await?;
            let mut current_offset = next_offset + segment.length;
            
            // 2. Extract any following pieces that are now sequential
            let mut to_flush = Vec::new();
            if let Some(pending) = self.pending_writes.get_mut(&id) {
                while let Some(data) = pending.remove(&current_offset) {
                    let len = data.len() as u64;
                    to_flush.push((current_offset, data));
                    current_offset += len;
                }
            }

            // 3. Flush the contiguous pieces
            for (offset, data) in to_flush {
                self.write_to_disk(id, offset, data).await?;
            }

            self.next_offsets.insert(id, current_offset);
        } else {
            // Out of order, buffer it
            debug!(%id, offset = %segment.offset, expected = %next_offset, "Buffering out-of-order piece");
            if let Some(pending) = self.pending_writes.get_mut(&id) {
                pending.insert(segment.offset, data);
            }
        }
        Ok(())
    }

    async fn write_to_disk(&mut self, id: TaskId, offset: u64, data: Bytes) -> Result<()> {
        let file = self.get_or_open_part_file(id).await?;
        file.seek(std::io::SeekFrom::Start(offset)).await?;
        file.write_all(&data).await?;
        // We don't flush every write to improve throughput; 
        // the OS or handle_complete will handle it.
        Ok(())
    }

    async fn flush_all_pending(&mut self, id: TaskId) -> Result<()> {
        if let Some(pending) = self.pending_writes.remove(&id) {
            for (offset, data) in pending {
                self.write_to_disk(id, offset, data).await?;
            }
        }
        Ok(())
    }

    async fn handle_complete(&mut self, id: TaskId) -> Result<()> {
        // 1. Close the handle
        self.handles.remove(&id);
        
        // 2. Perform atomic rename
        let base_path = self.task_paths.get(&id)
            .ok_or_else(|| Error::Storage("Task path not registered".to_string()))?;
        
        let part_path = get_part_path(base_path)?;
        
        info!(%id, from = ?part_path, to = ?base_path, "Performing atomic completion rename");
        fs::rename(&part_path, base_path).await?;
        
        // Notify Orchestrator
        let _ = self.completion_tx.send(id).await;
        
        Ok(())
    }

    async fn get_or_open_part_file(&mut self, id: TaskId) -> Result<&mut File> {
        if !self.handles.contains_key(&id) {
            let base_path = self.task_paths.get(&id)
                .ok_or_else(|| Error::Storage("Task path not registered".to_string()))?;
            
            let part_path = get_part_path(base_path)?;
            
            if let Some(parent) = part_path.parent() {
                fs::create_dir_all(parent).await?;
            }

            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(&part_path)
                .await?;
            
            self.handles.insert(id, file);
        }
        
        Ok(self.handles.get_mut(&id).unwrap())
    }
}

fn get_part_path(base_path: &Path) -> Result<PathBuf> {
    let mut part_path = base_path.to_path_buf();
    let mut filename = part_path.file_name()
        .ok_or_else(|| Error::Storage("Invalid filename".to_string()))?
        .to_os_string();
    filename.push(".part");
    part_path.set_file_name(filename);
    Ok(part_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_storage_engine_atomic_completion() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("final_file.bin");
        let _part_path = dir.path().join("final_file.bin.part");
        
        let (request_tx, request_rx) = mpsc::channel(1);
        let (completion_tx, _completion_rx) = mpsc::channel(1);
        let mut storage = StorageEngine::new(request_rx, completion_tx);
        let id = TaskId(100);
        storage.register_task(id, file_path.clone());

        tokio::spawn(async move {
            storage.run().await.unwrap();
        });

        let data = Bytes::from("atomic data");
        request_tx.send(StorageRequest::Write {
            task_id: id,
            segment: Segment { offset: 0, length: data.len() as u64 },
            data,
        }).await.unwrap();

        request_tx.send(StorageRequest::Complete(id)).await.unwrap();
        
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "atomic data");
    }

    #[tokio::test]
    async fn test_storage_engine_sequential_aggregation() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("seq_file.bin");
        
        let (request_tx, request_rx) = mpsc::channel(10);
        let (completion_tx, _completion_rx) = mpsc::channel(1);
        let mut storage = StorageEngine::new(request_rx, completion_tx);
        let id = TaskId(200);
        storage.register_task(id, file_path.clone());

        tokio::spawn(async move {
            storage.run().await.unwrap();
        });

        // 1. Send Piece 2 (Out of order)
        request_tx.send(StorageRequest::Write {
            task_id: id,
            segment: Segment { offset: 10, length: 5 },
            data: Bytes::from("world"),
        }).await.unwrap();

        // 2. Send Piece 1 (Now it becomes sequential)
        request_tx.send(StorageRequest::Write {
            task_id: id,
            segment: Segment { offset: 0, length: 10 },
            data: Bytes::from("hello_seq_"),
        }).await.unwrap();

        // 3. Trigger completion
        request_tx.send(StorageRequest::Complete(id)).await.unwrap();
        
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "hello_seq_world");
    }
}
