use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeedingConfig {
    pub min_ratio: f32,
    #[serde(rename = "max_seeding_time")]
    pub max_seeding_time_secs: u64,
    pub stop_on_either: bool,
}

impl Default for SeedingConfig {
    fn default() -> Self {
        Self {
            min_ratio: 1.0,
            max_seeding_time_secs: 0,
            stop_on_either: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum EncryptionPolicy {
    #[default]
    Prefer,
    Require,
    Disable,
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
    pub seeding: SeedingConfig,
    pub endgame_mode_enabled: bool,
    pub endgame_threshold_pieces: usize,
    pub endgame_threshold_percent: f32,
    pub streaming_metadata_pieces: usize,
    pub min_split_size_mb: u64,
    pub max_connections_per_torrent: usize,
    pub peer_id_prefix: String,
    pub peer_eviction_threshold: usize,
    pub peer_eviction_percent: f32,
    pub peer_idle_penalty_threshold_secs: f64,
    pub dht_save_interval_secs: u64,
    pub dht_ping_interval_secs: u64,
    pub dht_token_rotation_secs: u64,
    pub dht_query_interval_secs: u64,
    pub dht_query_timeout_secs: u64,
    pub tracker_polling_interval_secs: u64,
    pub lpd_announce_interval_secs: u64,
    pub choker_interval_secs: u64,
    pub encryption: EncryptionPolicy,
}

impl Default for BitTorrentConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_peers_per_torrent: 200,
            max_overall_peers: 500,
            request_pipeline_size: 50,
            dht_enabled: true,
            pex_enabled: true,
            lpd_enabled: false,
            seeding: SeedingConfig::default(),
            endgame_mode_enabled: true,
            endgame_threshold_pieces: 3,
            endgame_threshold_percent: 0.01,
            streaming_metadata_pieces: 4,
            min_split_size_mb: 20,
            max_connections_per_torrent: 200,
            peer_id_prefix: "-AR0001-".to_string(),
            peer_eviction_threshold: 500,
            peer_eviction_percent: 0.1,
            peer_idle_penalty_threshold_secs: 60.0,
            dht_save_interval_secs: 300,
            dht_ping_interval_secs: 600,
            dht_token_rotation_secs: 600,
            dht_query_interval_secs: 120,
            dht_query_timeout_secs: 5,
            tracker_polling_interval_secs: 60,
            lpd_announce_interval_secs: 300,
            choker_interval_secs: 10,
            encryption: EncryptionPolicy::Prefer,
        }
    }
}
