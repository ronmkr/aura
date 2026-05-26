//! peer_registry: Manages discovered peers and their states.

use crate::tracker::Peer;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Handshaked,
}

#[derive(Debug, Clone)]
pub struct PeerState {
    pub peer: Peer,
    pub state: ConnectionState,
    pub am_choking: bool,
    pub am_interested: bool,
    pub peer_choking: bool,
    pub peer_interested: bool,
    pub downloaded_bytes: u64,
    pub last_downloaded_bytes: u64,
    pub download_rate: f64,
}

#[derive(Debug)]
pub struct PeerRegistry {
    peers: HashMap<String, PeerState>, // Key: ip:port
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    pub fn add_peers(&mut self, new_peers: Vec<Peer>) -> usize {
        let mut added = 0;
        for p in new_peers {
            let addr = format!("{}:{}", p.ip, p.port);
            self.peers.entry(addr).or_insert_with(|| {
                added += 1;
                PeerState {
                    peer: p,
                    state: ConnectionState::Disconnected,
                    am_choking: true,
                    am_interested: false,
                    peer_choking: true,
                    peer_interested: false,
                    downloaded_bytes: 0,
                    last_downloaded_bytes: 0,
                    download_rate: 0.0,
                }
            });
        }
        added
    }

    pub fn get_peer_to_connect(&mut self) -> Option<Peer> {
        if let Some(ps) = self
            .peers
            .values_mut()
            .find(|ps| ps.state == ConnectionState::Disconnected)
        {
            ps.state = ConnectionState::Connecting;
            Some(ps.peer.clone())
        } else {
            None
        }
    }

    pub fn update_state(&mut self, addr: &str, state: ConnectionState) {
        if let Some(ps) = self.peers.get_mut(addr) {
            ps.state = state;
        }
    }

    pub fn add_downloaded(&mut self, addr: &str, bytes: u64) {
        if let Some(ps) = self.peers.get_mut(addr) {
            ps.downloaded_bytes += bytes;
        }
    }

    pub fn tick_rates(&mut self, elapsed_secs: f64) {
        for ps in self.peers.values_mut() {
            let bytes_in_interval = ps.downloaded_bytes.saturating_sub(ps.last_downloaded_bytes);
            ps.download_rate = bytes_in_interval as f64 / elapsed_secs;
            ps.last_downloaded_bytes = ps.downloaded_bytes;
        }
    }

    pub fn get_all_connected(&mut self) -> Vec<&mut PeerState> {
        self.peers
            .values_mut()
            .filter(|ps| {
                ps.state == ConnectionState::Handshaked || ps.state == ConnectionState::Connected
            })
            .collect()
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peer_registry_basics() {
        let mut registry = PeerRegistry::new();
        let p1 = Peer {
            id: None,
            ip: "1.1.1.1".to_string(),
            port: 80,
        };
        let p2 = Peer {
            id: None,
            ip: "2.2.2.2".to_string(),
            port: 80,
        };

        registry.add_peers(vec![p1.clone(), p2.clone()]);
        assert_eq!(registry.peer_count(), 2);

        let to_connect = registry.get_peer_to_connect().unwrap();
        assert!(to_connect.ip == "1.1.1.1" || to_connect.ip == "2.2.2.2");

        let to_connect2 = registry.get_peer_to_connect().unwrap();
        assert!(to_connect2.ip == "1.1.1.1" || to_connect2.ip == "2.2.2.2");
        assert_ne!(to_connect.ip, to_connect2.ip);
    }

    #[test]
    fn test_peer_registry_rates() {
        let mut registry = PeerRegistry::new();
        let p1 = Peer {
            id: None,
            ip: "1.1.1.1".to_string(),
            port: 80,
        };
        registry.add_peers(vec![p1]);

        let addr = "1.1.1.1:80";
        registry.update_state(addr, ConnectionState::Handshaked);
        registry.add_downloaded(addr, 1024);

        registry.tick_rates(1.0);
        let connected = registry.get_all_connected();
        assert_eq!(connected.len(), 1);
        assert_eq!(connected[0].download_rate, 1024.0);

        // Tick again with no new downloads
        registry.tick_rates(1.0);
        let connected2 = registry.get_all_connected();
        assert_eq!(connected2[0].download_rate, 0.0);
    }
}
