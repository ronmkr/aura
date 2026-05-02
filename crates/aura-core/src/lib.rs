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
    pub bittorrent: BitTorrentConfig,
    pub storage: StorageConfig,
    pub general: GeneralConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkConfig {
    pub interface: Option<String>,
    pub local_addr: Option<std::net::IpAddr>,
    pub listen_port: u16,
    pub dht_port: u16,
    pub rpc_port: u16,
    pub rpc_secret: Option<String>,
    pub user_agent: String,
    pub connect_timeout_secs: u64,
    pub tcp_keepalive_secs: u64,
    pub proxy: Option<String>,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            interface: None,
            local_addr: None,
            listen_port: 6881,
            dht_port: 6881,
            rpc_port: 6800,
            rpc_secret: None,
            user_agent: "Aura/0.1.0".to_string(),
            connect_timeout_secs: 30,
            tcp_keepalive_secs: 60,
            proxy: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BandwidthConfig {
    pub global_download_limit: u64, // 0 for unlimited
    pub global_upload_limit: u64,
    pub per_task_download_limit: u64,
    pub per_task_upload_limit: u64,
    pub max_concurrent_downloads: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BitTorrentConfig {
    pub enabled: bool,
    pub max_peers_per_torrent: usize,
    pub max_overall_peers: usize,
    pub request_pipeline_size: usize,
    pub dht_enabled: bool,
    pub pex_enabled: bool,
    pub lpd_enabled: bool,
    pub seed_ratio: f32,
    pub seed_time_mins: u32,
    pub endgame_mode_enabled: bool,
}

impl Default for BitTorrentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_peers_per_torrent: 50,
            max_overall_peers: 200,
            request_pipeline_size: 10,
            dht_enabled: true,
            pex_enabled: true,
            lpd_enabled: false,
            seed_ratio: 1.0,
            seed_time_mins: 0,
            endgame_mode_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub download_dir: String,
    pub cache_size_mb: u32,
    pub preallocate: bool,
    pub allocation_mode: String, // "none", "prealloc", "falloc"
    pub save_session_interval_secs: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            download_dir: ".".to_string(),
            cache_size_mb: 16,
            preallocate: true,
            allocation_mode: "falloc".to_string(),
            save_session_interval_secs: 10,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub log_level: String,
    pub log_path: Option<String>,
    pub check_integrity: bool,
    pub event_poll_interval_ms: u64,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            log_path: None,
            check_integrity: true,
            event_poll_interval_ms: 500,
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
