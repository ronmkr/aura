//! worker: Abstractions for protocol-specific data retrieval.

use crate::{Result, TaskId};
use async_trait::async_trait;
use bytes::Bytes;
use tokio::sync::mpsc;

pub mod http;
pub mod ftp;
pub mod builder;

pub use http::HttpWorker;
pub use ftp::FtpWorker;
pub use builder::WorkerBuilder;

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
    pub data: Bytes,
}

/// Metadata discovered about a resource (size, filename, etc.).
#[derive(Debug, Clone)]
pub struct Metadata {
    pub final_uri: String,
    pub total_length: Option<u64>,
    pub name: Option<String>,
}

/// The core trait for all protocol-specific downloaders.
#[async_trait]
pub trait ProtocolWorker: Send + Sync {
    /// Fetches a single segment of data.
    async fn fetch_segment(&self, task_id: TaskId, segment: Segment, progress: Option<ProgressSender>) -> Result<PieceData>;
    
    /// Returns the number of concurrent requests this worker can handle.
    fn available_capacity(&self) -> usize;
}
