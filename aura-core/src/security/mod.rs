pub mod alt_svc;
pub use alt_svc::{AltSvcCache, AltSvcPolicy};

use arc_swap::ArcSwap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HstsPolicy {
    pub domain: String,
    pub expiry: u64, // unix timestamp in seconds
    pub include_subdomains: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HstsCacheInner {
    pub policies: HashMap<String, HstsPolicy>,
}

#[derive(Debug, Clone)]
pub struct HstsCache {
    inner: Arc<RwLock<HstsCacheInner>>,
    config: Arc<ArcSwap<crate::Config>>,
}

impl Default for HstsCache {
    fn default() -> Self {
        Self::new(Arc::new(ArcSwap::from_pointee(crate::Config::default())))
    }
}

impl HstsCache {
    pub fn new(config: Arc<ArcSwap<crate::Config>>) -> Self {
        let path = Self::resolve_path(&config);
        let cache = Self::load(&path);
        Self {
            inner: Arc::new(RwLock::new(cache)),
            config,
        }
    }

    fn resolve_path(config: &ArcSwap<crate::Config>) -> std::path::PathBuf {
        let current_config = config.load();
        let base_dir = if let Some(ref sandbox) = current_config.storage.sandbox_root {
            std::path::PathBuf::from(sandbox)
        } else {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(std::path::PathBuf::from);
            home.map(|h| h.join(".aura"))
                .unwrap_or_else(|| std::path::PathBuf::from(".aura"))
        };
        base_dir.join("hsts.json")
    }

    pub fn load(path: &std::path::Path) -> HstsCacheInner {
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(cache) = serde_json::from_str::<HstsCacheInner>(&data) {
                    return cache;
                }
            }
        }
        HstsCacheInner::default()
    }

    pub async fn save(&self) {
        let path = Self::resolve_path(&self.config);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let inner = self.inner.read().await;
        if let Ok(data) = serde_json::to_string_pretty(&*inner) {
            let _ = std::fs::write(&path, data);
        }
    }

    pub async fn should_upgrade(&self, domain: &str) -> bool {
        let inner = self.inner.read().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Normalize domain
        let domain_lower = domain.to_lowercase();

        // Safeguard: Never upgrade localhost or loopback IP addresses
        if domain_lower == "127.0.0.1" || domain_lower == "localhost" || domain_lower == "::1" {
            return false;
        }

        // 1. Direct match
        if let Some(policy) = inner.policies.get(&domain_lower) {
            if policy.expiry > now {
                return true;
            }
        }

        // 2. Subdomain match
        let parts: Vec<&str> = domain_lower.split('.').collect();
        for i in 1..parts.len() {
            let parent_domain = parts[i..].join(".");
            if let Some(policy) = inner.policies.get(&parent_domain) {
                if policy.include_subdomains && policy.expiry > now {
                    return true;
                }
            }
        }

        false
    }

    pub async fn insert_policy(&self, domain: String, max_age: u64, include_subdomains: bool) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expiry = now + max_age;
        let domain_lower = domain.to_lowercase();

        {
            let mut inner = self.inner.write().await;
            inner.policies.insert(
                domain_lower.clone(),
                HstsPolicy {
                    domain: domain_lower,
                    expiry,
                    include_subdomains,
                },
            );
        }

        self.save().await;
    }

    pub async fn insert_header(&self, domain: String, header_value: &str) {
        if let Some((max_age, include_subdomains)) = parse_hsts_header(header_value) {
            self.insert_policy(domain, max_age, include_subdomains)
                .await;
        }
    }
}

/// Parses the Strict-Transport-Security header value (e.g. "max-age=31536000; includeSubDomains").
pub fn parse_hsts_header(value: &str) -> Option<(u64, bool)> {
    let mut max_age = None;
    let mut include_subdomains = false;

    for part in value.split(';') {
        let trimmed = part.trim();
        if trimmed.to_lowercase().starts_with("max-age=") {
            if let Ok(parsed) = trimmed["max-age=".len()..].trim().parse::<u64>() {
                max_age = Some(parsed);
            }
        } else if trimmed.eq_ignore_ascii_case("includeSubDomains") {
            include_subdomains = true;
        }
    }

    max_age.map(|age| (age, include_subdomains))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
