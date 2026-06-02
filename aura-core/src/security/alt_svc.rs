use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AltSvcPolicy {
    pub host: String,             // original host (lowercase)
    pub alt_protocol: String,     // e.g. "h3"
    pub alt_host: Option<String>, // Some alternative host or None (same host)
    pub alt_port: u16,
    pub expiry: u64, // unix timestamp
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AltSvcCacheInner {
    pub policies: HashMap<String, Vec<AltSvcPolicy>>,
}

#[derive(Debug, Clone)]
pub struct AltSvcCache {
    inner: Arc<RwLock<AltSvcCacheInner>>,
}

impl Default for AltSvcCache {
    fn default() -> Self {
        Self::new()
    }
}

impl AltSvcCache {
    pub fn new() -> Self {
        let cache = Self::load();
        Self {
            inner: Arc::new(RwLock::new(cache)),
        }
    }

    pub fn load() -> AltSvcCacheInner {
        let path = std::path::Path::new(".aura/alt_svc.json");
        if path.exists() {
            if let Ok(data) = std::fs::read_to_string(path) {
                if let Ok(cache) = serde_json::from_str::<AltSvcCacheInner>(&data) {
                    return cache;
                }
            }
        }
        AltSvcCacheInner::default()
    }

    pub async fn save(&self) {
        let inner = self.inner.read().await;
        let _ = std::fs::create_dir_all(".aura");
        if let Ok(data) = serde_json::to_string_pretty(&*inner) {
            let _ = std::fs::write(".aura/alt_svc.json", data);
        }
    }

    pub async fn get_alt_svc(&self, domain: &str) -> Option<AltSvcPolicy> {
        let inner = self.inner.read().await;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let domain_lower = domain.to_lowercase();

        // Safeguard: Never upgrade localhost or loopback IP addresses in production
        #[cfg(not(test))]
        if domain_lower == "127.0.0.1" || domain_lower == "localhost" || domain_lower == "::1" {
            return None;
        }

        if let Some(policies) = inner.policies.get(&domain_lower) {
            for policy in policies {
                if policy.expiry > now
                    && (policy.alt_protocol == "h3" || policy.alt_protocol.starts_with("h3-"))
                {
                    return Some(policy.clone());
                }
            }
        }
        None
    }

    pub async fn insert_policies(&self, domain: String, header_value: &str) {
        let domain_lower = domain.to_lowercase();
        #[cfg(not(test))]
        if domain_lower == "127.0.0.1" || domain_lower == "localhost" || domain_lower == "::1" {
            return;
        }

        if header_value.trim().eq_ignore_ascii_case("clear") {
            let mut inner = self.inner.write().await;
            inner.policies.remove(&domain_lower);
            // Drop write lock before calling async save to avoid deadlock or holding the lock too long
            drop(inner);
            self.save().await;
            return;
        }

        if let Some(mut parsed) = parse_alt_svc_header(header_value) {
            for policy in &mut parsed {
                policy.host = domain_lower.clone();
            }

            let mut inner = self.inner.write().await;
            inner.policies.insert(domain_lower, parsed);
            drop(inner);
            self.save().await;
        }
    }
}

/// Helper to parse the Alt-Svc header value.
pub fn parse_alt_svc_header(value: &str) -> Option<Vec<AltSvcPolicy>> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("clear") {
        return Some(Vec::new());
    }

    let mut policies = Vec::new();

    for part in value.split(',') {
        let trimmed_part = part.trim();
        if trimmed_part.is_empty() {
            continue;
        }

        let mut semi_split = trimmed_part.split(';');
        let Some(alt_authority_part) = semi_split.next() else {
            continue;
        };

        let mut eq_split = alt_authority_part.split('=');
        let Some(protocol) = eq_split.next() else {
            continue;
        };
        let protocol = protocol.trim();
        let Some(authority_quoted) = eq_split.next() else {
            continue;
        };
        let authority_quoted = authority_quoted.trim();

        let authority = if authority_quoted.starts_with('"') && authority_quoted.ends_with('"') {
            &authority_quoted[1..authority_quoted.len() - 1]
        } else {
            authority_quoted
        };

        let mut host_port_split = authority.split(':');
        let Some(host_part) = host_port_split.next() else {
            continue;
        };
        let host_part = host_part.trim();
        let Some(port_part) = host_port_split.next() else {
            continue;
        };
        let port_part = port_part.trim();

        let alt_host = if host_part.is_empty() {
            None
        } else {
            Some(host_part.to_string())
        };

        let Ok(alt_port) = port_part.parse::<u16>() else {
            continue;
        };

        let mut ma = 86400;

        for param in semi_split {
            let param_trimmed = param.trim();
            if param_trimmed.to_lowercase().starts_with("ma=") {
                if let Ok(parsed_ma) = param_trimmed["ma=".len()..].trim().parse::<u64>() {
                    ma = parsed_ma;
                }
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let expiry = now + ma;

        policies.push(AltSvcPolicy {
            host: String::new(),
            alt_protocol: protocol.to_string(),
            alt_host,
            alt_port,
            expiry,
        });
    }

    if policies.is_empty() {
        None
    } else {
        Some(policies)
    }
}

/// Helper to rewrite URLs for Alt-Svc connection racing/fallback.
pub fn rewrite_url_for_alt_svc(url_str: &str, policy: &AltSvcPolicy) -> Option<String> {
    let mut url = url::Url::parse(url_str).ok()?;
    if let Some(ref alt_host) = policy.alt_host {
        url.set_host(Some(alt_host)).ok()?;
    }
    url.set_port(Some(policy.alt_port)).ok()?;
    Some(url.to_string())
}

#[cfg(test)]
#[path = "alt_svc_tests.rs"]
mod tests;
