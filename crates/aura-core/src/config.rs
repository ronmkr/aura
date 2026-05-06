use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub network: NetworkConfig,
    pub bandwidth: BandwidthConfig,
    pub bittorrent: BitTorrentConfig,
    pub storage: StorageConfig,
    pub vpn: VpnConfig,
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
    pub max_redirects: usize,
    pub http_retry_count: u32,
    pub http_retry_delay_secs: u64,
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
            max_redirects: 20,
            http_retry_count: 5,
            http_retry_delay_secs: 2,
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
    pub max_active_tasks: usize,
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
    pub min_split_size_mb: u64,
    pub max_connections_per_torrent: usize,
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
            min_split_size_mb: 20,
            max_connections_per_torrent: 100,
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
    pub read_ahead_kb: u32,
    pub write_buffer_kb: u32,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            download_dir: ".".to_string(),
            cache_size_mb: 16,
            preallocate: true,
            allocation_mode: "falloc".to_string(),
            save_session_interval_secs: 10,
            read_ahead_kb: 128,
            write_buffer_kb: 256,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VpnConfig {
    pub type_name: Option<String>, // "openvpn", "wireguard"
    pub profile_path: Option<String>,
    pub auto_connect: bool,
    pub check_interval_secs: u64,
    pub force_tunnel: bool,
}

impl Default for VpnConfig {
    fn default() -> Self {
        Self {
            type_name: None,
            profile_path: None,
            auto_connect: false,
            check_interval_secs: 5,
            force_tunnel: true,
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
    pub daemon_mode: bool,
    pub theme: ThemeConfig,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            log_level: "info".to_string(),
            log_path: None,
            check_integrity: true,
            event_poll_interval_ms: 500,
            daemon_mode: false,
            theme: ThemeConfig::galactic(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub primary: String,
    pub accent: String,
    pub highlight: String,
    pub background: String,
    pub foreground: String,
    pub success: String,
    pub error: String,
    pub warning: String,
}

impl ThemeConfig {
    pub fn galactic() -> Self {
        Self {
            primary: "#0000FF".to_string(),   // Galactic Blue
            accent: "#00FFFF".to_string(),    // Nebula Cyan
            highlight: "#FFFF00".to_string(), // Star Yellow
            background: "#000000".to_string(),
            foreground: "#FFFFFF".to_string(),
            success: "#00FF00".to_string(),
            error: "#FF0000".to_string(),
            warning: "#FFFF00".to_string(),
        }
    }

    pub fn matrix() -> Self {
        Self {
            primary: "#003B00".to_string(),
            accent: "#00FF41".to_string(),
            highlight: "#008F11".to_string(),
            background: "#000000".to_string(),
            foreground: "#00FF41".to_string(),
            success: "#00FF41".to_string(),
            error: "#FF0000".to_string(),
            warning: "#008F11".to_string(),
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self::galactic()
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
