//! Aura-core: The high-performance download engine.
#![warn(clippy::cognitive_complexity)]
#![warn(clippy::type_complexity)]

use serde::{Deserialize, Serialize};
use std::fmt;

pub mod api;
pub mod bitfield;
pub mod dht;
pub mod glob;
pub mod hooks;
pub mod lpd;
pub mod magnet;
pub mod metalink;
pub mod nat;
pub mod net_util;
pub mod orchestrator;
pub mod peer_registry;
pub mod piece_picker;
pub mod power;
pub mod scrubber;
pub mod storage;
pub mod task;
pub mod throttler;
pub mod torrent;
pub mod tracker;
pub mod vpn;
pub mod worker;

pub mod config;
pub mod history;
pub mod security;

pub use api::{TaskEvent, TaskHandle};
pub use config::{CliOverrides, Config};
pub use history::{CompletedTaskRecord, HistoryManager};
pub use orchestrator::Engine;

/// Supported checksum algorithms for non-swarm integrity verification.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Checksum {
    Md5(String),
    Sha1(String),
    Sha256(String),
    Sha512(String),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum SeedingCompleteReason {
    RatioReached,
    TimeExpired,
}

/// Newtype for Download Task identifiers to ensure type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub u64);

pub const DEFAULT_TASK_NAME: &str = "unnamed";
pub const RPC_AUTH_HEADER: &str = "X-Aura-Token";
pub const JSONRPC_VERSION: &str = "2.0";

impl TaskId {
    pub fn random() -> Self {
        Self(rand::random())
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "task-{}", self.0)
    }
}

/// Identifier for multi-tenant isolation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub String);

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "tenant-{}", self.0)
    }
}

/// BitTorrent Info Hash (supports v1 20-byte and v2 32-byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum InfoHash {
    V1([u8; 20]),
    V2([u8; 32]),
}

impl InfoHash {
    pub fn as_v1(&self) -> Option<[u8; 20]> {
        match self {
            InfoHash::V1(h) => Some(*h),
            _ => None,
        }
    }

    /// Returns a 20-byte hash for the handshake.
    /// For v1, it's the 20-byte SHA-1 hash.
    /// For v2, it's the FIRST 20 bytes of the SHA-256 hash (as per BEP 52).
    pub fn for_handshake(&self) -> [u8; 20] {
        match self {
            InfoHash::V1(h) => *h,
            InfoHash::V2(h) => {
                let mut truncated = [0u8; 20];
                truncated.copy_from_slice(&h[..20]);
                truncated
            }
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        match self {
            InfoHash::V1(h) => h.to_vec(),
            InfoHash::V2(h) => h.to_vec(),
        }
    }

    pub fn to_magnet_urn(&self) -> String {
        match self {
            InfoHash::V1(h) => format!("urn:btih:{}", hex::encode(h)),
            InfoHash::V2(h) => format!("urn:btmh:1220{}", hex::encode(h)),
        }
    }

    pub fn matches_handshake(&self, handshake_hash: &[u8; 20]) -> bool {
        match self {
            InfoHash::V1(h) => h == handshake_hash,
            InfoHash::V2(h) => &h[..20] == handshake_hash,
        }
    }
}

/// Core error types for the Aura engine.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Captive portal detected: {0}")]
    CaptivePortal(String),

    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Worker error: {0}")]
    Worker(String),

    #[error("Task error for {0}: {1}")]
    Task(TaskId, String),

    #[error("Task already exists: {0}")]
    DuplicateTask(TaskId),

    #[error("Too many active tasks: limit {0} reached")]
    TooManyTasks(usize),

    #[error("Engine error: {0}")]
    Engine(String),

    #[error("Not modified")]
    NotModified,
}

/// A specialized Result type for Aura operations.
pub type Result<T> = std::result::Result<T, Error>;
