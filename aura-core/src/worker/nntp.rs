use crate::throttler::Throttler;
use crate::worker::{PieceData, ProgressSender, ProtocolWorker, Segment, WorkerOptions};
use crate::{Result, TaskId};
use async_trait::async_trait;
use bytes::BytesMut;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct NntpWorker {
    _options: WorkerOptions,
}

impl NntpWorker {
    pub fn new(options: WorkerOptions) -> Self {
        Self { _options: options }
    }
}

#[async_trait]
impl ProtocolWorker for NntpWorker {
    async fn fetch_segment(
        &self,
        _task_id: TaskId,
        _segment: Segment,
        _progress: Option<ProgressSender>,
        _storage_tx: Option<mpsc::Sender<crate::storage::StorageRequest>>,
        _throttler: Arc<Throttler>,
    ) -> Result<PieceData> {
        Err(crate::Error::Protocol("nntp unsupported".to_string()))
    }

    fn available_capacity(&self) -> usize {
        1
    }
}
