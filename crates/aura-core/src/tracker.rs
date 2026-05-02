//! tracker: Handles communication with BitTorrent trackers (HTTP and UDP).

use serde::{Deserialize, Serialize};
use crate::{Result, Error};
use crate::torrent::Torrent;
use url::Url;
use tokio::net::UdpSocket;
use tokio::time::{timeout, Duration};
use bytes::{Buf, BufMut, BytesMut};
use tracing::debug;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Peer {
    #[serde(rename = "peer id")]
    pub id: Option<serde_bencode::value::Value>,
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrackerResponse {
    pub interval: u32,
    pub peers: serde_bencode::value::Value,
    #[serde(rename = "min interval")]
    pub min_interval: Option<u32>,
    pub complete: Option<u32>,
    pub incomplete: Option<u32>,
}

pub struct TrackerClient {
    client: reqwest::Client,
    peer_id: [u8; 20],
    port: u16,
    local_addr: Option<std::net::IpAddr>,
}

impl TrackerClient {
    pub fn new(peer_id: [u8; 20], port: u16, local_addr: Option<std::net::IpAddr>) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static("Aura/0.1.0"));
        
        let mut builder = reqwest::Client::builder()
            .default_headers(headers);

        if let Some(addr) = local_addr {
            builder = builder.local_address(addr);
        }

        Self {
            client: builder.build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            peer_id,
            port,
            local_addr,
        }
    }

    pub async fn announce(&self, torrent: &Torrent) -> Result<Vec<Peer>> {
        let mut trackers = Vec::new();
        trackers.push(torrent.announce.clone());
        if let Some(list) = &torrent.announce_list {
            for tier in list {
                for t in tier {
                    if !trackers.contains(t) {
                        trackers.push(t.clone());
                    }
                }
            }
        }

        // Randomize order
        {
            use rand::seq::SliceRandom;
            let mut rng = rand::thread_rng();
            trackers.shuffle(&mut rng);
        }

        use futures_util::future::join_all;
        let mut futures = Vec::new();
        for tracker_url in trackers {
            futures.push(self.announce_to_url_wrapper(tracker_url, torrent));
        }

        let results = join_all(futures).await;
        let mut all_peers = Vec::new();
        let mut success = false;

        for res in results {
            if let Ok(peers) = res {
                all_peers.extend(peers);
                success = true;
            }
        }

        if success {
            Ok(all_peers)
        } else {
            Err(Error::Protocol("All tracker announces failed".to_string()))
        }
    }

    async fn announce_to_url_wrapper(&self, url_str: String, torrent: &Torrent) -> Result<Vec<Peer>> {
        match self.announce_to_url(&url_str, torrent).await {
            Ok(peers) => Ok(peers),
            Err(e) => {
                debug!(url = %url_str, error = %e, "Tracker announce failed");
                Err(e)
            }
        }
    }

    async fn announce_to_url(&self, url_str: &str, torrent: &Torrent) -> Result<Vec<Peer>> {
        if url_str.starts_with("udp://") {
            self.announce_udp(url_str, torrent).await
        } else {
            self.announce_http(url_str, torrent).await
        }
    }

    async fn announce_http(&self, url_str: &str, torrent: &Torrent) -> Result<Vec<Peer>> {
        let info_hash = torrent.info_hash()?;
        let info_hash_str: String = info_hash.iter().map(|b| format!("%{:02x}", b)).collect();
        let peer_id_str: String = self.peer_id.iter().map(|b| format!("%{:02x}", b)).collect();

        let mut url = Url::parse(url_str)
            .map_err(|e| Error::Protocol(format!("Invalid tracker URL: {}", e)))?;
        
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("port", &self.port.to_string());
            query.append_pair("uploaded", "0");
            query.append_pair("downloaded", "0");
            query.append_pair("left", &torrent.total_length().to_string());
            query.append_pair("compact", "1");
            query.append_pair("event", "started");
        }

        let mut final_url = url.to_string();
        final_url.push_str("&info_hash=");
        final_url.push_str(&info_hash_str);
        final_url.push_str("&peer_id=");
        final_url.push_str(&peer_id_str);

        debug!(url = %url_str, "Attempting HTTP tracker announce");
        let response = self.client.get(final_url).send().await
            .map_err(|e| Error::Protocol(format!("Tracker request failed: {}", e)))?;
        
        let status = response.status();
        if !status.is_success() {
            return Err(Error::Protocol(format!("Tracker returned error status: {}", status)));
        }

        let bytes = response.bytes().await
            .map_err(|e| Error::Protocol(format!("Failed to read tracker response: {}", e)))?;
        
        // Attempt to parse as a dictionary first
        let res_val: serde_bencode::value::Value = match serde_bencode::from_bytes(&bytes) {
            Ok(v) => v,
            Err(e) => {
                debug!(url = %url_str, error = %e, "Failed to bdecode tracker response");
                return Err(Error::Protocol(format!("Failed to bdecode tracker response: {}", e)));
            }
        };

        if let serde_bencode::value::Value::Dict(dict) = res_val {
            if let Some(serde_bencode::value::Value::Bytes(reason)) = dict.get(&b"failure reason".to_vec()) {
                let reason_str = String::from_utf8_lossy(reason).to_string();
                return Err(Error::Protocol(format!("Tracker reported failure: {}", reason_str)));
            }

            if let Some(peers) = dict.get(&b"peers".to_vec()) {
                return self.parse_peers(peers.clone());
            }
        }

        Err(Error::Protocol("Invalid tracker response format (missing peers)".to_string()))
    }

    async fn announce_udp(&self, url_str: &str, torrent: &Torrent) -> Result<Vec<Peer>> {
        let url = Url::parse(url_str)
            .map_err(|e| Error::Protocol(format!("Invalid UDP tracker URL: {}", e)))?;
        let host = url.host_str().ok_or_else(|| Error::Protocol("Missing host in UDP tracker URL".to_string()))?;
        let port = url.port().ok_or_else(|| Error::Protocol("Missing port in UDP tracker URL".to_string()))?;
        
        let addrs = tokio::net::lookup_host(format!("{}:{}", host, port)).await
            .map_err(|e| Error::Protocol(format!("Failed to resolve UDP tracker host: {}", e)))?
            .collect::<Vec<_>>();
        
        if addrs.is_empty() {
            return Err(Error::Protocol("Could not resolve UDP tracker host".to_string()));
        }

        let mut last_error = Error::Protocol("All UDP attempts failed".to_string());

        for addr in addrs {
            debug!(url = %url_str, %addr, "Attempting UDP tracker announce");
            
            let socket = match crate::net_util::bind_udp_bound(0, None, self.local_addr).await {
                Ok(s) => s,
                Err(e) => {
                    debug!(%addr, error = %e, "Failed to bind UDP socket");
                    last_error = e;
                    continue;
                }
            };

            if let Err(e) = socket.connect(addr).await {
                debug!(%addr, error = %e, "Failed to connect UDP socket");
                last_error = Error::Protocol(format!("Failed to connect UDP socket: {}", e));
                continue;
            }

            match self.announce_udp_addr(&socket, addr, torrent).await {
                Ok(peers) => return Ok(peers),
                Err(e) => {
                    debug!(%addr, error = %e, "UDP announce failed for address");
                    last_error = e;
                }
            }
        }

        Err(last_error)
    }

    async fn announce_udp_addr(&self, socket: &UdpSocket, addr: std::net::SocketAddr, torrent: &Torrent) -> Result<Vec<Peer>> {
        let mut connection_id: u64 = 0x41727101980; // Initial magic for connect
        let mut transaction_id: u32 = rand::random();
        
        // 1. Connect Phase
        let mut buf = [0u8; 1024];
        let mut connected = false;

        for n in 0..3 {
            let timeout_duration = Duration::from_secs(5 * (1 << n));
            
            let mut connect_req = BytesMut::with_capacity(16);
            connect_req.put_u64(0x41727101980); // Connection ID magic
            connect_req.put_u32(0); // Action: Connect
            connect_req.put_u32(transaction_id);

            socket.send(&connect_req).await
                .map_err(|e| Error::Protocol(format!("Failed to send UDP connect request: {}", e)))?;

            match timeout(timeout_duration, socket.recv_from(&mut buf)).await {
                Ok(Ok((len, _))) => {
                    if len < 16 {
                        return Err(Error::Protocol("UDP connect response too short".to_string()));
                    }

                    let mut res_slice = &buf[..len];
                    let action = res_slice.get_u32();
                    let received_tid = res_slice.get_u32();
                    
                    if action == 3 {
                        let error_msg = String::from_utf8_lossy(res_slice).to_string();
                        return Err(Error::Protocol(format!("UDP tracker error: {}", error_msg)));
                    }

                    if action != 0 || received_tid != transaction_id {
                        continue;
                    }

                    connection_id = res_slice.get_u64();
                    connected = true;
                    break;
                }
                _ => {
                    debug!(%addr, attempt = n, "UDP connect timeout/error");
                    transaction_id = rand::random();
                }
            }
        }

        if !connected {
            return Err(Error::Protocol("UDP connect failed after retries".to_string()));
        }

        // 2. Announce Phase
        let transaction_id: u32 = rand::random();
        for n in 0..3 {
            let timeout_duration = Duration::from_secs(5 * (1 << n));

            let mut announce_req = BytesMut::with_capacity(98);
            announce_req.put_u64(connection_id);
            announce_req.put_u32(1); // Action: Announce
            announce_req.put_u32(transaction_id);
            announce_req.put_slice(&torrent.info_hash()?);
            announce_req.put_slice(&self.peer_id);
            announce_req.put_u64(0); // Downloaded
            announce_req.put_u64(torrent.total_length()); // Left
            announce_req.put_u64(0); // Uploaded
            announce_req.put_u32(2); // Event: Started
            announce_req.put_u32(0); // IP: 0 (default)
            announce_req.put_u32(rand::random()); // Key
            announce_req.put_i32(-1); // Num want (default)
            announce_req.put_u16(self.port);

            socket.send(&announce_req).await
                .map_err(|e| Error::Protocol(format!("Failed to send UDP announce request: {}", e)))?;

            match timeout(timeout_duration, socket.recv_from(&mut buf)).await {
                Ok(Ok((len, _))) => {
                    if len < 20 {
                        return Err(Error::Protocol("UDP announce response too short".to_string()));
                    }

                    let mut res_slice = &buf[..len];
                    let action = res_slice.get_u32();
                    let received_tid = res_slice.get_u32();

                    if action == 3 {
                        let error_msg = String::from_utf8_lossy(res_slice).to_string();
                        return Err(Error::Protocol(format!("UDP tracker error: {}", error_msg)));
                    }

                    if action != 1 || received_tid != transaction_id {
                        continue;
                    }

                    let _interval = res_slice.get_u32();
                    let _leechers = res_slice.get_u32();
                    let _seeders = res_slice.get_u32();

                    return self.parse_compact_peers_raw(res_slice);
                }
                _ => {
                    debug!(%addr, attempt = n, "UDP announce timeout/error");
                }
            }
        }

        Err(Error::Protocol("UDP announce failed after retries".to_string()))
    }

    fn parse_peers(&self, peers_val: serde_bencode::value::Value) -> Result<Vec<Peer>> {
        match peers_val {
            serde_bencode::value::Value::List(list) => {
                let mut peers = Vec::new();
                for p in list {
                    if let serde_bencode::value::Value::Dict(dict) = p {
                        let ip = if let Some(serde_bencode::value::Value::Bytes(b)) = dict.get(&b"ip".to_vec()) {
                            String::from_utf8_lossy(b).to_string()
                        } else {
                            continue;
                        };
                        let port = if let Some(serde_bencode::value::Value::Int(p)) = dict.get(&b"port".to_vec()) {
                            *p as u16
                        } else {
                            continue;
                        };
                        peers.push(Peer {
                            id: dict.get(&b"peer id".to_vec()).cloned(),
                            ip,
                            port,
                        });
                    }
                }
                Ok(peers)
            }
            serde_bencode::value::Value::Bytes(bytes) => {
                self.parse_compact_peers_raw(&bytes)
            }
            _ => Err(Error::Protocol("Unknown peers format".to_string())),
        }
    }

    fn parse_compact_peers_raw(&self, bytes: &[u8]) -> Result<Vec<Peer>> {
        let mut peers = Vec::new();
        for chunk in bytes.chunks_exact(6) {
            let ip = format!("{}.{}.{}.{}", chunk[0], chunk[1], chunk[2], chunk[3]);
            let port = u16::from_be_bytes([chunk[4], chunk[5]]);
            peers.push(Peer {
                id: None,
                ip,
                port,
            });
        }
        Ok(peers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_compact_peers() {
        let client = TrackerClient::new([0; 20], 6881);
        let bytes = vec![127, 0, 0, 1, 0x1a, 0xe1]; // 127.0.0.1:6881
        let peers = client.parse_compact_peers_raw(&bytes).unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].ip, "127.0.0.1");
        assert_eq!(peers[0].port, 6881);
    }

    #[test]
    fn test_parse_non_compact_peers() {
        let client = TrackerClient::new([0; 20], 6881);
        let mut peer_dict = std::collections::HashMap::new();
        peer_dict.insert(b"ip".to_vec(), serde_bencode::value::Value::Bytes(b"127.0.0.1".to_vec()));
        peer_dict.insert(b"port".to_vec(), serde_bencode::value::Value::Int(6881));
        
        let peers_val = serde_bencode::value::Value::List(vec![
            serde_bencode::value::Value::Dict(peer_dict)
        ]);
        
        let peers = client.parse_peers(peers_val).unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].ip, "127.0.0.1");
        assert_eq!(peers[0].port, 6881);
    }
}
