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
}

impl Default for HstsCache {
    fn default() -> Self {
        Self::new()
    }
}

impl HstsCache {
    pub fn new() -> Self {
        let cache = Self::load();
        Self {
            inner: Arc::new(RwLock::new(cache)),
        }
    }

    pub fn load() -> HstsCacheInner {
        let path = std::path::Path::new(".aura/hsts.json");
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
        let inner = self.inner.read().await;
        let _ = std::fs::create_dir_all(".aura");
        if let Ok(data) = serde_json::to_string_pretty(&*inner) {
            let _ = std::fs::write(".aura/hsts.json", data);
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
mod tests {
    use super::*;

    #[test]
    fn test_parse_hsts_header() {
        assert_eq!(
            parse_hsts_header("max-age=31536000"),
            Some((31536000, false))
        );
        assert_eq!(
            parse_hsts_header("max-age=600; includeSubDomains"),
            Some((600, true))
        );
        assert_eq!(
            parse_hsts_header("includeSubDomains; max-age=1200"),
            Some((1200, true))
        );
        assert_eq!(parse_hsts_header("invalid-header"), None);
    }

    #[tokio::test]
    async fn test_hsts_cache_upgrades() {
        let cache = HstsCache::new();
        let domain = "secure-example.com".to_string();

        // Initially no policy, so should not upgrade
        assert!(!cache.should_upgrade(&domain).await);

        // Add HSTS policy with 60s max-age
        cache.insert_policy(domain.clone(), 60, false).await;
        assert!(cache.should_upgrade(&domain).await);

        // Subdomain should not upgrade since include_subdomains is false
        assert!(!cache.should_upgrade("sub.secure-example.com").await);

        // Expiration check (simulated with 0 max-age)
        cache.insert_policy(domain.clone(), 0, false).await;
        // Small delay just to be completely sure expiry is not in exact same second
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!cache.should_upgrade(&domain).await);
    }

    #[tokio::test]
    async fn test_hsts_subdomain_matching() {
        let cache = HstsCache::new();
        let domain = "parent.com".to_string();

        cache.insert_policy(domain.clone(), 300, true).await;
        assert!(cache.should_upgrade(&domain).await);
        // Subdomains should also be upgraded
        assert!(cache.should_upgrade("sub.parent.com").await);
        assert!(cache.should_upgrade("deep.sub.parent.com").await);

        // Mismatched domain should not upgrade
        assert!(!cache.should_upgrade("otherparent.com").await);
    }
}
