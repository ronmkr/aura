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
        checksum: Option<crate::Checksum>,
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

#[derive(Debug, Clone)]
pub enum StorageEvent {
    Completed(TaskId),
    Error(TaskId, String),
}

pub struct StorageEngine {
    pub(crate) request_rx: mpsc::Receiver<StorageRequest>,
    pub(crate) completion_tx: mpsc::Sender<StorageEvent>,
    pub(crate) task_paths: HashMap<TaskId, PathBuf>,
    pub(crate) task_checksums: HashMap<TaskId, crate::Checksum>,
    pub(crate) handles: HashMap<TaskId, File>,
    pub(crate) pending_writes: HashMap<TaskId, BTreeMap<u64, BytesMut>>,
    pub(crate) next_offsets: HashMap<TaskId, u64>,
    pub(crate) pool: BufferPool,
    pub(crate) db: sled::Db,
}

impl StorageEngine {
    pub fn new(
        request_rx: mpsc::Receiver<StorageRequest>,
        completion_tx: mpsc::Sender<StorageEvent>,
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
            task_checksums: HashMap::new(),
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

    pub fn register_task(
        &mut self,
        id: TaskId,
        path: PathBuf,
        _total_length: u64,
        checksum: Option<crate::Checksum>,
    ) {
        self.task_paths.insert(id, path);
        if let Some(c) = checksum {
            self.task_checksums.insert(id, c);
        }
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
                    checksum,
                } => {
                    self.register_task(task_id, path, total_length, checksum);
                    if let Err(e) = self.preallocate_task(task_id, total_length).await {
                        error!(%task_id, error = %e, "Failed to pre-allocate file");
                        let _ = self
                            .completion_tx
                            .send(StorageEvent::Error(task_id, e.to_string()))
                            .await;
                    }
                }
                StorageRequest::Write {
                    task_id,
                    segment,
                    data,
                } => {
                    if let Err(e) = self.handle_write(task_id, segment, data).await {
                        error!(%task_id, error = %e, "Failed to write data");
                        let _ = self
                            .completion_tx
                            .send(StorageEvent::Error(task_id, e.to_string()))
                            .await;
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

                    // Flush all buffered data to disk and update metadata of the .part file
                    if let Some(file) = self.handles.get_mut(&task_id) {
                        if let Err(e) = file.sync_all().await {
                            error!(%task_id, error = %e, "Failed to sync file data on completion");
                        }
                    }

                    // Close the write handle before verification to ensure all data is committed
                    // and to allow clean read-only access.
                    self.handles.remove(&task_id);

                    // Perform integrity verification if a checksum was provided
                    if let Err(e) = self.verify_checksum(task_id).await {
                        error!(%task_id, error = %e, "Integrity verification failed");
                        let _ = self
                            .completion_tx
                            .send(StorageEvent::Error(task_id, e.to_string()))
                            .await;
                        continue;
                    }

                    if let Err(e) = self.handle_complete(task_id).await {
                        error!(%task_id, error = %e, "Failed to complete task");
                        let _ = self
                            .completion_tx
                            .send(StorageEvent::Error(task_id, e.to_string()))
                            .await;
                    } else {
                        let _ = self
                            .completion_tx
                            .send(StorageEvent::Completed(task_id))
                            .await;
                    }
                }
            }
        }

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
