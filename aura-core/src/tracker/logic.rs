//! tracker: Implementation of BitTorrent HTTP and UDP trackers.

use crate::torrent::Torrent;
use crate::{Error, Result};
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use url::Url;

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
}

impl TrackerClient {
    pub fn new(
        peer_id: [u8; 20],
        port: u16,
        local_addr: Option<std::net::IpAddr>,
        user_agent: Option<String>,
        proxy: Option<String>,
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

        let mut builder = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(10));

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
        }
    }

    pub async fn announce(&self, torrent: &Torrent) -> Result<Vec<Peer>> {
        let mut trackers = Vec::new();
        trackers.push(torrent.announce.clone());
        if let Some(announce_list) = &torrent.announce_list {
            for list in announce_list {
                for url in list {
                    if !trackers.contains(url) {
                        trackers.push(url.clone());
                    }
                }
            }
        }

        let mut futures = Vec::new();
        for url in &trackers {
            futures.push(self.announce_single(url.clone(), torrent));
        }

        let results = join_all(futures).await;
        let mut all_peers = Vec::new();
        let mut success = false;

        for (i, res) in results.into_iter().enumerate() {
            let url = &trackers[i];
            match res {
                Ok(peers) => {
                    tracing::info!(url = %url, count = peers.len(), "Tracker returned peers");
                    all_peers.extend(peers);
                    success = true;
                }
                Err(e) => {
                    tracing::debug!(url = %url, error = %e, "Tracker announce failed");
                }
            }
        }

        if success {
            tracing::info!(
                total = all_peers.len(),
                "Discovered peers from all trackers"
            );
            Ok(all_peers)
        } else {
            Err(Error::Protocol(
                "All tracker announcements failed".to_string(),
            ))
        }
    }

    async fn announce_single(&self, url: String, torrent: &Torrent) -> Result<Vec<Peer>> {
        if url.starts_with("http") {
            self.announce_http(&url, torrent).await
        } else if url.starts_with("udp") {
            self.announce_udp(&url, torrent).await
        } else {
            Err(Error::Protocol(format!(
                "Unsupported tracker protocol: {}",
                url
            )))
        }
    }

    async fn announce_http(&self, url_str: &str, torrent: &Torrent) -> Result<Vec<Peer>> {
        let info_hash = if let Some(h2) = torrent.info_hash_v2()? {
            let mut truncated = [0u8; 20];
            truncated.copy_from_slice(&h2[..20]);
            truncated
        } else {
            torrent
                .info_hash_v1()?
                .ok_or_else(|| Error::Protocol("No info hash available".to_string()))?
        };

        let info_hash_encoded: String = info_hash.iter().map(|b| format!("%{:02x}", b)).collect();
        let peer_id_encoded: String = self.peer_id.iter().map(|b| format!("%{:02x}", b)).collect();

        let url = Url::parse(url_str)
            .map_err(|e| Error::Protocol(format!("Invalid tracker URL: {}", e)))?;

        let query = format!(
            "info_hash={}&peer_id={}&port={}&uploaded=0&downloaded=0&left={}&compact=1&event=started",
            info_hash_encoded,
            peer_id_encoded,
            self.port,
            torrent.total_length()
        );

        let final_url = if url.query().is_some() {
            format!("{}&{}", url_str, query)
        } else {
            format!("{}?{}", url_str, query)
        };

        let bytes = self
            .client
            .get(&final_url)
            .send()
            .await
            .map_err(|e| Error::Protocol(format!("Tracker request failed: {}", e)))?
            .bytes()
            .await
            .map_err(|e| Error::Protocol(format!("Failed to read tracker response: {}", e)))?;

        let res_val: serde_bencode::value::Value = serde_bencode::from_bytes(&bytes)
            .map_err(|e| Error::Protocol(format!("Failed to bdecode tracker response: {}", e)))?;

        if let serde_bencode::value::Value::Dict(dict) = res_val {
            if let Some(serde_bencode::value::Value::Bytes(reason)) =
                dict.get(b"failure reason".as_slice())
            {
                let reason_str = String::from_utf8_lossy(reason).to_string();
                return Err(Error::Protocol(format!(
                    "Tracker reported failure: {}",
                    reason_str
                )));
            }

            if let Some(peers) = dict.get(b"peers".as_slice()) {
                return self.parse_peers(peers.clone());
            }
        }

        Err(Error::Protocol(
            "Invalid tracker response format (missing peers)".to_string(),
        ))
    }

    pub(crate) fn parse_peers(&self, peers_val: serde_bencode::value::Value) -> Result<Vec<Peer>> {
        match peers_val {
            serde_bencode::value::Value::List(list) => {
                let mut peers = Vec::new();
                for p in list {
                    if let serde_bencode::value::Value::Dict(dict) = p {
                        let ip = if let Some(serde_bencode::value::Value::Bytes(b)) =
                            dict.get(b"ip".as_slice())
                        {
                            String::from_utf8_lossy(b).to_string()
                        } else {
                            continue;
                        };
                        let port = if let Some(serde_bencode::value::Value::Int(p)) =
                            dict.get(b"port".as_slice())
                        {
                            *p as u16
                        } else {
                            continue;
                        };
                        peers.push(Peer {
                            id: dict.get(b"peer id".as_slice()).cloned(),
                            ip,
                            port,
                        });
                    }
                }
                Ok(peers)
            }
            serde_bencode::value::Value::Bytes(bytes) => self.parse_compact_peers_raw(&bytes),
            _ => Err(Error::Protocol("Invalid peers format".to_string())),
        }
    }

    pub(crate) fn parse_compact_peers_raw(&self, bytes: &[u8]) -> Result<Vec<Peer>> {
        let mut peers = Vec::new();
        for chunk in bytes.chunks_exact(6) {
            let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
            let port = u16::from_be_bytes([chunk[4], chunk[5]]);
            peers.push(Peer {
                id: None,
                ip: ip.to_string(),
                port,
            });
        }
        Ok(peers)
    }
}
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
