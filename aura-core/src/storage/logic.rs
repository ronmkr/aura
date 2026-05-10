use super::ops::get_part_path;
use crate::buffer_pool::BufferPool;
use crate::worker::Segment;
use crate::{Error, Result, TaskId};
use bytes::{Bytes, BytesMut};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use tokio::fs::{self, File, OpenOptions};
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

#[derive(Debug)]
pub enum StorageRequest {
    RegisterTask {
        task_id: TaskId,
        path: PathBuf,
        total_length: u64,
    },
    Write {
        task_id: TaskId,
        segment: Segment,
        data: BytesMut,
    },
    Read {
        task_id: TaskId,
        segment: Segment,
        reply_tx: oneshot::Sender<Result<Bytes>>,
    },
    StoreV2Metadata {
        // pieces_root (32 bytes) -> piece_layers (concatenated 32-byte hashes)
        layers: HashMap<[u8; 32], Vec<u8>>,
    },
    Complete(TaskId),
}

pub struct StorageEngine {
    pub(crate) request_rx: mpsc::Receiver<StorageRequest>,
    pub(crate) completion_tx: mpsc::Sender<TaskId>,
    pub(crate) task_paths: HashMap<TaskId, PathBuf>,
    pub(crate) handles: HashMap<TaskId, File>,
    pub(crate) pending_writes: HashMap<TaskId, BTreeMap<u64, BytesMut>>,
    pub(crate) next_offsets: HashMap<TaskId, u64>,
    pub(crate) pool: BufferPool,
    pub(crate) db: sled::Db,
}

impl StorageEngine {
    pub fn new(
        request_rx: mpsc::Receiver<StorageRequest>,
        completion_tx: mpsc::Sender<TaskId>,
        db_path: Option<PathBuf>,
    ) -> Self {
        let pool = BufferPool::new(1024 * 1024, 10);

        let db = if let Some(path) = db_path {
            sled::open(&path).expect("Failed to open metadata database")
        } else {
            sled::Config::new()
                .temporary(true)
                .open()
                .expect("Failed to open temp database")
        };

        Self {
            request_rx,
            completion_tx,
            task_paths: HashMap::new(),
            handles: HashMap::new(),
            pending_writes: HashMap::new(),
            next_offsets: HashMap::new(),
            pool: pool.clone(),
            db,
        }
    }

    pub fn get_pool(&self) -> BufferPool {
        self.pool.clone()
    }

    pub fn get_db(&self) -> sled::Db {
        self.db.clone()
    }

    pub fn register_task(&mut self, id: TaskId, path: PathBuf, _total_length: u64) {
        self.task_paths.insert(id, path);
        self.next_offsets.entry(id).or_insert(0);
        self.pending_writes.entry(id).or_default();
    }

    pub(crate) async fn preallocate_task(&mut self, id: TaskId, length: u64) -> Result<()> {
        if length == 0 {
            return Ok(());
        }
        let file = self.get_or_open_part_file(id).await?;

        let file_clone = file.try_clone().await?.into_std().await;
        let length_clone = length;
        let _ = tokio::task::spawn_blocking(move || {
            let _ = crate::storage::sys::harden_file(&file_clone, length_clone);
        })
        .await;

        file.set_len(length).await.map_err(Error::from)
    }

    pub async fn run(mut self) -> Result<()> {
        info!("Storage Engine started");

        while let Some(req) = self.request_rx.recv().await {
            match req {
                StorageRequest::RegisterTask {
                    task_id,
                    path,
                    total_length,
                } => {
                    self.register_task(task_id, path, total_length);
                    if let Err(e) = self.preallocate_task(task_id, total_length).await {
                        error!(%task_id, error = %e, "Failed to pre-allocate file");
                    }
                }
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
                StorageRequest::StoreV2Metadata { layers } => {
                    for (root, data) in layers {
                        if let Err(e) = self.db.insert(root, data) {
                            error!(?root, error = %e, "Failed to store v2 piece layers");
                        }
                    }
                    let _ = self.db.flush();
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
        let mut storage = StorageEngine::new(request_rx, completion_tx, None);
        let id = TaskId(100);
        storage.register_task(id, file_path.clone(), 0);

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
                data: data.into(),
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
        let mut storage = StorageEngine::new(request_rx, completion_tx, None);
        let id = TaskId(200);
        storage.register_task(id, file_path.clone(), 0);

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
                data: Bytes::from("world").into(),
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
                data: Bytes::from("hello_seq_").into(),
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
