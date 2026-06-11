use crate::throttler::Throttler;
use crate::worker::builder::WorkerOptions;
use crate::worker::{PieceData, ProgressSender, ProtocolWorker, Segment};
use crate::{Result, TaskId};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct S3Worker {
    _options: WorkerOptions,
}

impl S3Worker {
    pub fn new(options: WorkerOptions) -> Self {
        Self { _options: options }
    }
}

#[async_trait]
impl ProtocolWorker for S3Worker {
    async fn fetch_segment(
        &self,
        _task_id: TaskId,
        _segment: Segment,
        _progress: Option<ProgressSender>,
        _storage_tx: Option<mpsc::Sender<crate::storage::StorageRequest>>,
        _throttler: Arc<Throttler>,
    ) -> Result<PieceData> {
        Err(crate::Error::Protocol("s3 unsupported".to_string()))
    }

    fn available_capacity(&self) -> usize {
        1
    }
}
