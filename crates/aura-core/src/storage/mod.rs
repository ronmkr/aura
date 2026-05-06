pub mod ops;

use crate::buffer_pool::BufferPool;
use crate::worker::Segment;
use crate::{Error, Result, TaskId};
use bytes::Bytes;
use ops::get_part_path;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use tokio::fs::{self, File, OpenOptions};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

#[derive(Debug)]
pub enum StorageRequest {
    Write {
        task_id: TaskId,
        segment: Segment,
        data: Bytes,
    },
    Read {
        task_id: TaskId,
        segment: Segment,
        reply_tx: oneshot::Sender<Result<Bytes>>,
    },
    Complete(TaskId),
}

pub struct StorageEngine {
    pub(crate) request_rx: mpsc::Receiver<StorageRequest>,
    pub(crate) completion_tx: mpsc::Sender<TaskId>,
    pub(crate) task_paths: HashMap<TaskId, PathBuf>,
    pub(crate) handles: HashMap<TaskId, File>,
    pub(crate) pending_writes: HashMap<TaskId, BTreeMap<u64, Bytes>>,
    pub(crate) next_offsets: HashMap<TaskId, u64>,
    pub(crate) _pool: BufferPool,
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
            _pool: BufferPool::new(1024 * 1024, 10),
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
                StorageRequest::Write {
                    task_id,
                    segment,
                    data,
                } => {
                    if let Err(e) = self.handle_write(task_id, segment, data).await {
                        error!(%task_id, error = %e, "Failed to write data");
                    }
                }
                StorageRequest::Read {
                    task_id,
                    segment,
                    reply_tx,
                } => {
                    let res = self.handle_read(task_id, segment).await;
                    let _ = reply_tx.send(res);
                }
                StorageRequest::Complete(task_id) => {
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

    pub(crate) async fn get_or_open_part_file(&mut self, id: TaskId) -> Result<&mut File> {
        if !self.handles.contains_key(&id) {
            let base_path = self
                .task_paths
                .get(&id)
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

        Ok(self
            .handles
            .get_mut(&id)
            .expect("File handle must exist after open/insert"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_storage_engine_atomic_completion() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("final_file.bin");
        let (request_tx, request_rx) = mpsc::channel(1);
        let (completion_tx, _completion_rx) = mpsc::channel(1);
        let mut storage = StorageEngine::new(request_rx, completion_tx);
        let id = TaskId(100);
        storage.register_task(id, file_path.clone());

        tokio::spawn(async move {
            storage.run().await.unwrap();
        });

        let data = Bytes::from("atomic data");
        request_tx
            .send(StorageRequest::Write {
                task_id: id,
                segment: Segment {
                    offset: 0,
                    length: data.len() as u64,
                },
                data,
            })
            .await
            .unwrap();

        request_tx.send(StorageRequest::Complete(id)).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path).unwrap();
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

        request_tx
            .send(StorageRequest::Write {
                task_id: id,
                segment: Segment {
                    offset: 10,
                    length: 5,
                },
                data: Bytes::from("world"),
            })
            .await
            .unwrap();

        request_tx
            .send(StorageRequest::Write {
                task_id: id,
                segment: Segment {
                    offset: 0,
                    length: 10,
                },
                data: Bytes::from("hello_seq_"),
            })
            .await
            .unwrap();

        request_tx.send(StorageRequest::Complete(id)).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        assert!(file_path.exists());
        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "hello_seq_world");
    }
}
