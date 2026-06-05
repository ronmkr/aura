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
    pub min_split_size_mb: u64,
    pub max_connections_per_torrent: usize,
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
            min_split_size_mb: 20,
            max_connections_per_torrent: 200,
        }
    }
}
