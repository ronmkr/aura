use serde::{Deserialize, Serialize};

/// Represents the current lifecycle state of a Download Task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DownloadPhase {
    MetadataExchange,
    Downloading,
    Verifying,
    Paused,
    Complete,
    Error,
    Degraded,
    Waiting,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskType {
    Http,
    BitTorrent,
    Ftp,
    S3,
    GDrive,
    Nntp,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FollowOnAction {
    AutoStartTorrent,
    AutoStartMetalink,
    Custom(String),
}
