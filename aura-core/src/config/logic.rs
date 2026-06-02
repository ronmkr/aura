use crate::{Error, Result};
use serde::{Deserialize, Serialize};

// Import and re-export all sub-configs from their dedicated sibling modules (Facade Pattern)
pub use super::bandwidth::BandwidthConfig;
pub use super::bittorrent::BitTorrentConfig;
pub use super::general::{CredentialConfig, GeneralConfig, ThemeConfig};
pub use super::hooks::HookConfig;
pub use super::network::{NetworkConfig, ResolverConfig, StructuredResolverConfig};
pub use super::resource_mapping::{
    ConflictPolicy, MappingCondition, MappingRule, ResourceMappingConfig,
};
pub use super::storage::StorageConfig;
pub use super::vpn::VpnConfig;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub network: NetworkConfig,
    pub bandwidth: BandwidthConfig,
    pub bittorrent: BitTorrentConfig,
    pub storage: StorageConfig,
    pub resource_mapping: ResourceMappingConfig,
    pub vpn: VpnConfig,
    pub hooks: HookConfig,
    pub general: GeneralConfig,
    pub credentials: CredentialConfig,
}

impl Config {
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| Error::Config(format!("Failed to read config file: {}", e)))?;
        toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse TOML config: {}", e)))
    }
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
