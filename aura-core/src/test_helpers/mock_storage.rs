use crate::worker::Segment;
use crate::{Result, TaskId};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

type WriteLog = Vec<(TaskId, Segment, Vec<u8>)>;
type ReadCache = HashMap<(TaskId, u64, u64), Vec<u8>>;

/// A mock implementation of StorageDispatch for testing protocol workers in isolation.
pub struct MockStorage {
    pub writes: Arc<Mutex<WriteLog>>,
    pub reads: Arc<Mutex<ReadCache>>,
    pub completed: Arc<Mutex<Vec<TaskId>>>,
    pub pressure_threshold: Arc<Mutex<Option<usize>>>,
}

impl Default for MockStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl MockStorage {
    pub fn new() -> Self {
        Self {
            writes: Arc::new(Mutex::new(Vec::new())),
            reads: Arc::new(Mutex::new(HashMap::new())),
            completed: Arc::new(Mutex::new(Vec::new())),
            pressure_threshold: Arc::new(Mutex::new(None)),
        }
    }
}

#[async_trait]
impl crate::storage::StorageDispatch for MockStorage {
    async fn register_task(
        &self,
        _task_id: TaskId,
        _path: std::path::PathBuf,
        _total_length: u64,
        _checksum: Option<crate::Checksum>,
        _padding_ranges: Vec<crate::task::Range>,
    ) -> Result<()> {
        Ok(())
    }

    async fn submit_write(
        &self,
        task_id: TaskId,
        segment: Segment,
        data: BytesMut,
        _guard: Option<crate::orchestrator::resource_governor::MemoryGuard>,
        _generation: Option<u64>,
    ) -> Result<()> {
        if let Some(threshold) = *self.pressure_threshold.lock().await {
            let writes = self.writes.lock().await;
            if writes.len() >= threshold {
                return Err(crate::Error::Storage(
                    "Mock: backpressure triggered".to_string(),
                ));
            }
        }
        let mut writes = self.writes.lock().await;
        writes.push((task_id, segment, data.to_vec()));
        Ok(())
    }

    async fn submit_read(&self, task_id: TaskId, segment: Segment) -> Result<Bytes> {
        let reads = self.reads.lock().await;
        let key = (task_id, segment.offset, segment.length);
        if let Some(data) = reads.get(&key) {
            Ok(Bytes::copy_from_slice(data))
        } else {
            Err(crate::Error::Storage("Mock: block not found".to_string()))
        }
    }

    async fn complete(&self, task_id: TaskId) -> Result<()> {
        let mut completed = self.completed.lock().await;
        completed.push(task_id);
        Ok(())
    }

    async fn store_v2_metadata(&self, _layers: HashMap<[u8; 32], Vec<u8>>) -> Result<()> {
        Ok(())
    }

    async fn store_merkle_layer(
        &self,
        _pieces_root: [u8; 32],
        _index: u32,
        _hashes: Vec<[u8; 32]>,
    ) -> Result<()> {
        Ok(())
    }
}
