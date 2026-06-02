use serde::{Deserialize, Serialize};

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
            max_peers_per_torrent: 200,
            max_overall_peers: 500,
            request_pipeline_size: 50,
            dht_enabled: true,
            pex_enabled: true,
            lpd_enabled: false,
            seed_ratio: 1.0,
            seed_time_mins: 0,
            endgame_mode_enabled: true,
            min_split_size_mb: 20,
            max_connections_per_torrent: 200,
        }
    }
}
