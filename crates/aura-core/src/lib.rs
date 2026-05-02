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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub network: NetworkConfig,
    pub bandwidth: BandwidthConfig,
    pub seeding: SeedingConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    pub interface: Option<String>,
    pub local_addr: Option<std::net::IpAddr>,
    pub listen_port: u16,
    pub dht_port: u16,
    pub rpc_port: u16,
    pub user_agent: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            interface: None,
            local_addr: None,
            listen_port: 6881,
            dht_port: 6881,
            rpc_port: 6800,
            user_agent: "Aura/0.1.0".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BandwidthConfig {
    pub global_download_limit: u64, // 0 for unlimited
    pub global_upload_limit: u64,
    pub per_task_download_limit: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeedingConfig {
    pub enabled: bool,
    pub seed_ratio: f32,
    pub seed_time_mins: u32,
}

impl Default for SeedingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            seed_ratio: 1.0,
            seed_time_mins: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub download_dir: String,
    pub cache_size_mb: u32,
    pub preallocate: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            download_dir: ".".to_string(),
            cache_size_mb: 16,
            preallocate: true,
        }
    }
}

impl Config {
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read config file: {}", e)))?;
        toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse TOML config: {}", e)))
    }
}

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
