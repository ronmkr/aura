use crate::{Error, Result};
use serde::{Deserialize, Serialize};

// Import and re-export all sub-configs from their dedicated sibling modules (Facade Pattern)
pub use super::bandwidth::BandwidthConfig;
pub use super::bittorrent::{BitTorrentConfig, SeedingConfig};
pub use super::bulk::BulkConfig;
pub use super::general::{CredentialConfig, GeneralConfig, ThemeConfig};
pub use super::hooks::HookConfig;
pub use super::limits::LimitsConfig;
pub use super::network::{NetworkConfig, ResolverConfig, StructuredResolverConfig};
pub use super::notifications::NotificationConfig;
pub use super::resource_mapping::{
    ConflictPolicy, MappingCondition, MappingRule, ResourceMappingConfig,
};
pub use super::storage::StorageConfig;
pub use super::tui::TuiConfig;
pub use super::vpn::VpnConfig;

#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    pub download_dir: Option<String>,
    pub limit: Option<u64>,
    pub proxy: Option<String>,
    pub bind_address: Option<String>,
    pub rpc_port: Option<u16>,
    pub rpc_secret: Option<String>,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub network: NetworkConfig,
    pub bandwidth: BandwidthConfig,
    pub bittorrent: BitTorrentConfig,
    pub storage: StorageConfig,
    pub notifications: NotificationConfig,
    pub bulk: BulkConfig,
    pub tui: TuiConfig,
    pub resource_mapping: ResourceMappingConfig,
    pub vpn: VpnConfig,
    pub hooks: HookConfig,
    pub general: GeneralConfig,
    pub credentials: CredentialConfig,
    pub limits: LimitsConfig,
    #[serde(skip)]
    pub config_path: Option<std::path::PathBuf>,
}

impl Config {
    pub fn from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(&path)
            .map_err(|e| Error::Config(format!("Failed to read config file: {}", e)))?;
        let mut config: Self = toml::from_str(&content)
            .map_err(|e| Error::Config(format!("Failed to parse TOML config: {}", e)))?;
        config.config_path = Some(path.as_ref().to_path_buf());
        Ok(config)
    }

    pub fn resolve_path(custom_path: Option<&str>) -> Option<std::path::PathBuf> {
        if let Some(path_str) = custom_path {
            return Some(std::path::PathBuf::from(path_str));
        }

        let pwd_path = std::path::PathBuf::from("Aura.toml");
        if pwd_path.exists() {
            return Some(pwd_path);
        }

        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from);

        if let Some(path) = home {
            #[cfg(windows)]
            {
                let appdata = std::env::var_os("APPDATA")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| path.join("AppData").join("Roaming"));
                let win_path = appdata.join("aura").join("Aura.toml");
                if win_path.exists() {
                    return Some(win_path);
                }
            }
            #[cfg(not(windows))]
            {
                let unix_path = path.join(".config").join("aura").join("Aura.toml");
                if unix_path.exists() {
                    return Some(unix_path);
                }
            }
        }

        None
    }

    pub fn load_resolved(custom_path: Option<&str>) -> Result<Self> {
        let mut merged_value = toml::Value::Table(toml::map::Map::new());
        let mut resolved_paths = Vec::new();
        let mut custom_used = false;

        // 1. User config directory (lowest priority)
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from);

        if let Some(path) = home {
            #[cfg(windows)]
            {
                let appdata = std::env::var_os("APPDATA")
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| path.join("AppData").join("Roaming"));
                let win_path = appdata.join("aura").join("Aura.toml");
                if win_path.exists() {
                    resolved_paths.push(win_path);
                }
            }
            #[cfg(not(windows))]
            {
                let unix_path = path.join(".config").join("aura").join("Aura.toml");
                if unix_path.exists() {
                    resolved_paths.push(unix_path);
                }
            }
        }

        // 2. Working directory config (medium priority)
        let pwd_path = std::path::PathBuf::from("Aura.toml");
        if pwd_path.exists() {
            resolved_paths.push(pwd_path);
        }

        // 3. Custom path (highest priority)
        if let Some(path_str) = custom_path {
            resolved_paths.push(std::path::PathBuf::from(path_str));
            custom_used = true;
        }

        let mut last_path = None;
        for path in resolved_paths {
            let content = std::fs::read_to_string(&path).map_err(|e| {
                Error::Config(format!(
                    "Failed to read config file '{}': {}",
                    path.display(),
                    e
                ))
            })?;
            let val: toml::Value = toml::from_str(&content).map_err(|e| {
                Error::Config(format!(
                    "Failed to parse TOML in '{}': {}",
                    path.display(),
                    e
                ))
            })?;
            merge_toml_values(&mut merged_value, val);
            last_path = Some(path);
        }

        if last_path.is_none() && !custom_used {
            return Ok(Self::default());
        }

        let mut config: Self = merged_value.try_into().map_err(|e| {
            Error::Config(format!("Failed to deserialize merged configuration: {}", e))
        })?;

        config.config_path = last_path;
        Ok(config)
    }

    pub fn apply_cli_overrides(&mut self, overrides: CliOverrides) {
        if let Some(d) = overrides.download_dir {
            self.storage.download_dir = d;
        }
        if let Some(l) = overrides.limit {
            self.bandwidth.global_download_limit = l;
        }
        if let Some(p) = overrides.proxy {
            self.network.proxy = Some(p);
        }
        if let Some(addr) = overrides.bind_address {
            if let Ok(ip) = addr.parse() {
                self.network.bind_address = ip;
            }
        }
        if let Some(port) = overrides.rpc_port {
            self.network.rpc_port = port;
        }
        if let Some(secret) = overrides.rpc_secret {
            self.network.rpc_secret = Some(secret);
        }
        if let Some(cert) = overrides.tls_cert {
            self.network.tls_cert = Some(cert);
        }
        if let Some(key) = overrides.tls_key {
            self.network.tls_key = Some(key);
        }
    }

    pub fn rpc_secret_path() -> std::path::PathBuf {
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from);

        let mut path = match home {
            Some(h) => h,
            None => std::path::PathBuf::from("."),
        };
        path.push(".aura");
        path.push("rpc_secret");
        path
    }

    pub fn resolve_rpc_secret(provided: Option<String>) -> Option<String> {
        if let Some(s) = provided {
            return Some(s);
        }

        let path = Self::rpc_secret_path();
        if path.exists() {
            std::fs::read_to_string(&path)
                .ok()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        } else {
            None
        }
    }

    pub fn resolve_local_addr(&self) -> Option<std::net::IpAddr> {
        if let Some(addr) = self.network.local_addr {
            return Some(addr);
        }

        if let Some(ref iface) = self.network.interface {
            use local_ip_address::list_afinet_netifas;
            return list_afinet_netifas()
                .ok()
                .and_then(|ifas: Vec<(String, std::net::IpAddr)>| {
                    ifas.into_iter()
                        .find(|(name, _)| *name == *iface)
                        .map(|(_, ip)| ip)
                });
        }

        None
    }
}

fn merge_toml_values(base: &mut toml::Value, overrides: toml::Value) {
    if let (Some(base_table), Some(overrides_table)) = (base.as_table_mut(), overrides.as_table()) {
        for (key, val) in overrides_table {
            if base_table.contains_key(key) {
                let base_val = base_table.get_mut(key).unwrap();
                if base_val.is_table() && val.is_table() {
                    merge_toml_values(base_val, val.clone());
                } else {
                    *base_val = val.clone();
                }
            } else {
                base_table.insert(key.clone(), val.clone());
            }
        }
    }
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
