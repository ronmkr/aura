//! Aura-core: The high-performance download engine.
use std::fmt;
use serde::{Deserialize, Serialize};

pub mod orchestrator;
pub mod task;
pub mod worker;
pub mod storage;
pub mod buffer_pool;
pub mod bitfield;
pub mod piece_picker;
pub mod bt_worker;
pub mod torrent;
pub mod tracker;
pub mod peer_registry;
pub mod bt_task;
pub mod throttler;
pub mod dht;
pub mod nat;
pub mod glob;
pub mod net_util;

pub mod config;

pub use config::Config;

/// Newtype for Download Task identifiers to ensure type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(pub u64);

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "task-{}", self.0)
    }
}

/// Core error types for the Aura engine.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Task not found: {0}")]
    TaskNotFound(TaskId),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

/// A specialized Result type for Aura operations.
pub type Result<T> = std::result::Result<T, Error>;
