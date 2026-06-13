#[cfg(feature = "nntp")]
mod connection;
#[cfg(feature = "nntp")]
mod worker;

#[cfg(feature = "nntp")]
pub use worker::NntpWorker;

#[cfg(not(feature = "nntp"))]
pub struct NntpWorker {
    _options: crate::worker::builder::WorkerOptions,
}

#[cfg(not(feature = "nntp"))]
impl NntpWorker {
    pub fn new(options: crate::worker::builder::WorkerOptions) -> Self {
        Self { _options: options }
    }

    pub async fn resolve_metadata(&self) -> crate::Result<crate::worker::Metadata> {
        Err(crate::Error::Protocol(
            "NNTP feature not enabled".to_string(),
        ))
    }
}

#[cfg(not(feature = "nntp"))]
#[async_trait::async_trait]
impl crate::worker::ProtocolWorker for NntpWorker {
    async fn fetch_segment(
        &self,
        _task_id: crate::TaskId,
        _segment: crate::worker::Segment,
        _progress: Option<crate::worker::ProgressSender>,
        _storage_client: Option<std::sync::Arc<dyn crate::storage::StorageDispatch>>,
        _throttler: std::sync::Arc<crate::throttler::Throttler>,
    ) -> crate::Result<crate::worker::PieceData> {
        Err(crate::Error::UnsupportedProtocol(
            "NNTP feature not enabled".to_string(),
        ))
    }

    fn available_capacity(&self) -> usize {
        1
    }
}

#[cfg(test)]
#[cfg(feature = "nntp")]
#[path = "nntp/tests.rs"]
mod tests;
