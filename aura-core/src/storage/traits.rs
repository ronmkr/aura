use crate::worker::Segment;
use crate::{Result, TaskId};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use std::collections::HashMap;

/// Handles dispatching storage requests to the storage engine.
#[async_trait]
pub trait StorageDispatch: Send + Sync {
    /// Registers a new task with the storage engine.
    async fn register_task(
        &self,
        task_id: TaskId,
        path: std::path::PathBuf,
        total_length: u64,
        checksum: Option<crate::Checksum>,
        padding_ranges: Vec<crate::task::Range>,
    ) -> Result<()>;

    /// Submits a write request for a segment of data.
    async fn submit_write(
        &self,
        task_id: TaskId,
        segment: Segment,
        data: BytesMut,
        guard: Option<crate::orchestrator::resource_governor::MemoryGuard>,
        generation: Option<u64>,
    ) -> Result<()>;

    /// Submits a read request for a segment of data.
    async fn submit_read(&self, task_id: TaskId, segment: Segment) -> Result<Bytes>;

    /// Finalizes a task, ensuring all data is flushed to disk.
    async fn complete(&self, task_id: TaskId) -> Result<()>;

    /// Stores V2 BitTorrent metadata.
    async fn store_v2_metadata(&self, layers: HashMap<[u8; 32], Vec<u8>>) -> Result<()>;

    /// Stores a layer of the Merkle tree for BitTorrent v2.
    async fn store_merkle_layer(
        &self,
        pieces_root: [u8; 32],
        index: u32,
        hashes: Vec<[u8; 32]>,
    ) -> Result<()>;
}

/// A concrete client implementation of StorageDispatch that sends requests over a channel.
#[derive(Clone)]
pub struct StorageClient {
    pub(crate) tx: tokio::sync::mpsc::Sender<super::StorageRequest>,
}

impl StorageClient {
    pub fn new(tx: tokio::sync::mpsc::Sender<super::StorageRequest>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl StorageDispatch for StorageClient {
    async fn register_task(
        &self,
        task_id: TaskId,
        path: std::path::PathBuf,
        total_length: u64,
        checksum: Option<crate::Checksum>,
        padding_ranges: Vec<crate::task::Range>,
    ) -> Result<()> {
        self.tx
            .send(super::StorageRequest::RegisterTask {
                task_id,
                path,
                total_length,
                checksum,
                padding_ranges,
            })
            .await
            .map_err(|e| crate::Error::Storage(format!("Failed to send RegisterTask: {}", e)))
    }

    async fn submit_write(
        &self,
        task_id: TaskId,
        segment: Segment,
        data: BytesMut,
        guard: Option<crate::orchestrator::resource_governor::MemoryGuard>,
        generation: Option<u64>,
    ) -> Result<()> {
        self.tx
            .send(super::StorageRequest::Write {
                task_id,
                segment,
                data,
                guard,
                generation,
            })
            .await
            .map_err(|e| crate::Error::Storage(format!("Failed to send Write: {}", e)))
    }

    async fn submit_read(&self, task_id: TaskId, segment: Segment) -> Result<Bytes> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(super::StorageRequest::Read {
                task_id,
                segment,
                reply_tx,
            })
            .await
            .map_err(|e| crate::Error::Storage(format!("Failed to send Read: {}", e)))?;

        reply_rx.await.map_err(|_| {
            crate::Error::Storage("Storage engine shut down before reading".to_string())
        })?
    }

    async fn complete(&self, task_id: TaskId) -> Result<()> {
        self.tx
            .send(super::StorageRequest::Complete(task_id))
            .await
            .map_err(|e| crate::Error::Storage(format!("Failed to send Complete: {}", e)))
    }

    async fn store_v2_metadata(&self, layers: HashMap<[u8; 32], Vec<u8>>) -> Result<()> {
        self.tx
            .send(super::StorageRequest::StoreV2Metadata { layers })
            .await
            .map_err(|e| crate::Error::Storage(format!("Failed to send StoreV2Metadata: {}", e)))
    }

    async fn store_merkle_layer(
        &self,
        pieces_root: [u8; 32],
        index: u32,
        hashes: Vec<[u8; 32]>,
    ) -> Result<()> {
        self.tx
            .send(super::StorageRequest::StoreMerkleLayer {
                pieces_root,
                index,
                hashes,
            })
            .await
            .map_err(|e| crate::Error::Storage(format!("Failed to send StoreMerkleLayer: {}", e)))
    }
}
