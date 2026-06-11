//! tracker: Implementation of BitTorrent HTTP and UDP trackers.

use crate::torrent::Torrent;
use serde::{Deserialize, Serialize};

pub mod http;
pub mod http_scrape;
pub mod parsing;
pub mod udp;
pub mod udp_scrape;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub id: Option<serde_bencode::value::Value>,
    pub ip: String,
    pub port: u16,
}

pub struct TrackerClient {
    pub(crate) client: reqwest::Client,
    pub(crate) peer_id: [u8; 20],
    pub(crate) port: u16,
    pub(crate) local_addr: Option<std::net::IpAddr>,
    pub(crate) _user_agent: Option<String>,
    pub(crate) proxy: Option<String>,
    pub(crate) tracker_tiers:
        std::sync::Mutex<std::collections::HashMap<[u8; 20], Vec<Vec<String>>>>,
    pub(crate) config: Option<std::sync::Arc<arc_swap::ArcSwap<crate::Config>>>,
}

impl TrackerClient {
    pub fn new(
        peer_id: [u8; 20],
        port: u16,
        local_addr: Option<std::net::IpAddr>,
        user_agent: Option<String>,
        proxy: Option<String>,
        config: Option<std::sync::Arc<arc_swap::ArcSwap<crate::Config>>>,
    ) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        let ua = user_agent
            .clone()
            .unwrap_or_else(|| "Aura/0.1.0".to_string());
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_str(&ua)
                .unwrap_or_else(|_| reqwest::header::HeaderValue::from_static("Aura/0.1.0")),
        );

        let timeout_secs = config
            .as_ref()
            .map(|c| c.load().network.tracker_timeout_secs)
            .unwrap_or(10);

        let mut builder = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(timeout_secs));

        if let Some(addr) = local_addr {
            builder = builder.local_address(addr);
        }

        if let Some(ref p) = proxy {
            if let Ok(proxy_obj) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy_obj);
            }
        }

        Self {
            client: builder.build().unwrap_or_else(|_| reqwest::Client::new()),
            peer_id,
            port,
            local_addr,
            _user_agent: user_agent,
            proxy,
            tracker_tiers: std::sync::Mutex::new(std::collections::HashMap::new()),
            config,
        }
    }

    pub async fn scrape(&self, torrent: &Torrent) -> crate::Result<(u32, u32, u32)> {
        use futures_util::future::join_all;
        let mut urls = std::collections::HashSet::new();
        if let Some(announce_list) = &torrent.announce_list {
            for tier in announce_list {
                for url in tier {
                    if !url.is_empty() {
                        urls.insert(url.clone());
                    }
                }
            }
        }
        if !torrent.announce.is_empty() {
            urls.insert(torrent.announce.clone());
        }

        if urls.is_empty() {
            return Err(crate::Error::Protocol(
                "No tracker URLs available for scrape".to_string(),
            ));
        }

        let mut futures = Vec::new();
        for url in urls {
            futures.push(self.scrape_single(url, torrent));
        }

        let results = join_all(futures).await;
        let mut max_complete = 0;
        let mut max_incomplete = 0;
        let mut max_downloaded = 0;
        let mut success = false;

        for (complete, incomplete, downloaded) in results.into_iter().flatten() {
            success = true;
            if complete > max_complete {
                max_complete = complete;
            }
            if incomplete > max_incomplete {
                max_incomplete = incomplete;
            }
            if downloaded > max_downloaded {
                max_downloaded = downloaded;
            }
        }

        if success {
            Ok((max_complete, max_incomplete, max_downloaded))
        } else {
            Err(crate::Error::Protocol(
                "All scrape attempts failed".to_string(),
            ))
        }
    }

    async fn scrape_single(
        &self,
        url: String,
        torrent: &Torrent,
    ) -> crate::Result<(u32, u32, u32)> {
        if url.starts_with("http") {
            self.scrape_http(&url, torrent).await
        } else if url.starts_with("udp") {
            self.scrape_udp(&url, torrent).await
        } else {
            Err(crate::Error::Protocol(format!(
                "Unsupported tracker protocol for scrape: {}",
                url
            )))
        }
    }
}
