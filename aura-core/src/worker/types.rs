//! worker: Abstractions for protocol-specific data retrieval.

use crate::{Result, TaskId};
use async_trait::async_trait;
use bytes::BytesMut;
use tokio::sync::mpsc;

pub use super::builder::WorkerBuilder;
pub use super::ftp::FtpWorker;
pub use super::http::HttpWorker;

pub type ProgressSender = mpsc::UnboundedSender<u64>;

/// Represents a single byte range to be fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub offset: u64,
    pub length: u64,
}

/// The result of a successful segment fetch.
#[derive(Debug, Clone)]
pub struct PieceData {
    pub segment: Segment,
    pub data: BytesMut,
}

/// Metadata discovered about a resource (size, filename, etc.).
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub final_uri: String,
    pub total_length: Option<u64>,
    pub name: Option<String>,
    pub range_supported: bool,
    pub padding_ranges: Vec<crate::task::Range>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

use crate::throttler::Throttler;
use std::sync::Arc;

/// The core trait for all protocol-specific downloaders.
#[async_trait]
pub trait ProtocolWorker: Send + Sync {
    /// Fetches a single segment of data.
    async fn fetch_segment(
        &self,
        task_id: TaskId,
        segment: Segment,
        progress: Option<ProgressSender>,
        storage_client: Option<Arc<dyn crate::storage::StorageDispatch>>,
        throttler: Arc<Throttler>,
    ) -> Result<PieceData>;

    /// Returns the number of concurrent requests this worker can handle.
    fn available_capacity(&self) -> usize;
}
