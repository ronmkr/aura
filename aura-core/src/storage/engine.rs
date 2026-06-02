use crate::worker::Segment;
use crate::{Result, TaskId};
use bytes::{Bytes, BytesMut};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::sync::{mpsc, oneshot};
use tracing::{error, info};

#[derive(Debug)]
pub enum StorageRequest {
    RegisterTask {
        task_id: TaskId,
        path: PathBuf,
        total_length: u64,
        checksum: Option<crate::Checksum>,
        padding_ranges: Vec<crate::task::Range>,
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
    StoreMerkleLayer {
        pieces_root: [u8; 32],
        index: u32,
        hashes: Vec<[u8; 32]>,
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
    pub(crate) task_padding_ranges: HashMap<TaskId, Vec<crate::task::Range>>,
    pub(crate) handles: lru::LruCache<TaskId, File>,
    pub(crate) pending_writes: HashMap<TaskId, BTreeMap<u64, BytesMut>>,
    pub(crate) dirty_buffers: HashMap<TaskId, Vec<(u64, BytesMut)>>,
    pub(crate) dirty_sizes: HashMap<TaskId, usize>,
    pub(crate) next_offsets: HashMap<TaskId, u64>,
    pub(crate) db: sled::Db,
    pub(crate) scheduler: super::scheduler::IoScheduler,
    pub(crate) config: Option<Arc<arc_swap::ArcSwap<crate::Config>>>,
}

impl StorageEngine {
    pub fn new(
        request_rx: mpsc::Receiver<StorageRequest>,
        completion_tx: mpsc::Sender<StorageEvent>,
        db_path: Option<PathBuf>,
        config: Option<Arc<arc_swap::ArcSwap<crate::Config>>>,
    ) -> Self {
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
            task_padding_ranges: HashMap::new(),
            handles: lru::LruCache::new(std::num::NonZeroUsize::new(256).unwrap()),
            pending_writes: HashMap::new(),
            dirty_buffers: HashMap::new(),
            dirty_sizes: HashMap::new(),
            next_offsets: HashMap::new(),
            db,
            scheduler: super::scheduler::IoScheduler::new(),
            config,
        }
    }

    pub fn get_db(&self) -> sled::Db {
        self.db.clone()
    }

    pub async fn run(mut self) -> Result<()> {
        info!("Storage Engine started");

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));

        loop {
            let has_tasks = !self.scheduler.is_empty();

            tokio::select! {
                req_opt = self.request_rx.recv() => {
                    let req = match req_opt {
                        Some(r) => r,
                        None => break, // Channel closed
                    };

                    match req {
                        StorageRequest::RegisterTask {
                            task_id,
                            path,
                            total_length,
                            checksum,
                            padding_ranges,
                        } => {
                            self.register_task(task_id, path, total_length, checksum, padding_ranges)
                                .await;
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
                        StorageRequest::StoreMerkleLayer {
                            pieces_root,
                            index,
                            hashes,
                        } => {
                            let mut key = Vec::with_capacity(36);
                            key.extend_from_slice(&pieces_root);
                            key.extend_from_slice(&index.to_be_bytes());

                            let mut data = Vec::with_capacity(hashes.len() * 32);
                            for hash in hashes {
                                data.extend_from_slice(&hash);
                            }

                            if let Err(e) = self.db.insert(key, data) {
                                error!(?pieces_root, index, error = %e, "Failed to store v2 Merkle layer");
                            }
                            let _ = self.db.flush();
                        }
                        StorageRequest::Complete(task_id) => {
                            if let Err(e) = self.handle_complete(task_id).await {
                                error!(%task_id, error = %e, "Failed to complete task");
                                let _ = self
                                    .completion_tx
                                    .send(StorageEvent::Error(task_id, e.to_string()))
                                    .await;
                            }
                        }
                    }
                }
                _ = interval.tick() => {
                    // Generational epoch flush
                    let tasks: Vec<TaskId> = self.dirty_sizes.iter()
                        .filter_map(|(&id, &size)| if size > 0 { Some(id) } else { None })
                        .collect();

                    for task_id in tasks {
                        if let Err(e) = self.flush_dirty_buffer(task_id).await {
                            error!(%task_id, error = %e, "Generational flush failed");
                        }
                    }
                }
                _ = std::future::ready(()), if has_tasks => {
                    if let Some(task) = self.scheduler.pop() {
                        if let Err(e) = self.execute_io_task(task).await {
                            error!("Scheduled I/O task failed: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

pub(crate) fn get_non_padding_subranges_impl(
    padding_ranges: &[crate::task::Range],
    offset: u64,
    length: u64,
) -> Vec<crate::task::Range> {
    if padding_ranges.is_empty() {
        return vec![crate::task::Range {
            start: offset,
            end: offset + length,
        }];
    }

    let mut current_offset = offset;
    let end_offset = offset + length;
    let mut result = Vec::new();

    while current_offset < end_offset {
        // Find the next padding range that overlaps with [current_offset, end_offset)
        let next_pad = padding_ranges
            .iter()
            .filter(|r| r.end > current_offset && r.start < end_offset)
            .min_by_key(|r| r.start);

        match next_pad {
            Some(pad) => {
                if pad.start > current_offset {
                    // There's a gap of real data before the padding starts
                    result.push(crate::task::Range {
                        start: current_offset,
                        end: pad.start,
                    });
                }
                // Skip the padding part
                current_offset = pad.end;
            }
            None => {
                // No more overlapping padding ranges
                result.push(crate::task::Range {
                    start: current_offset,
                    end: end_offset,
                });
                break;
            }
        }
    }
    result
}

/// Identifies the sub-ranges of a buffer that ARE padding and should be zeroed out.
pub(crate) fn get_padding_subranges_impl(
    padding_ranges: &[crate::task::Range],
    offset: u64,
    length: u64,
) -> Vec<crate::task::Range> {
    let mut result = Vec::new();
    let end_offset = offset + length;

    for pad in padding_ranges {
        // Calculate the intersection of [pad.start, pad.end) and [offset, end_offset)
        let intersect_start = std::cmp::max(pad.start, offset);
        let intersect_end = std::cmp::min(pad.end, end_offset);

        if intersect_start < intersect_end {
            result.push(crate::task::Range {
                start: intersect_start,
                end: intersect_end,
            });
        }
    }
    result
}
