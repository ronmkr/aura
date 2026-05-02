//! peer_registry: Manages discovered peers and their states.

use std::collections::HashMap;
use crate::tracker::Peer;

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
                }
            });
        }
        added
    }

    pub fn get_peer_to_connect(&self) -> Option<Peer> {
        self.peers.values()
            .find(|ps| ps.state == ConnectionState::Disconnected)
            .map(|ps| ps.peer.clone())
    }

    pub fn update_state(&mut self, addr: &str, state: ConnectionState) {
        if let Some(ps) = self.peers.get_mut(addr) {
            ps.state = state;
        }
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
mod tests {
    use super::*;

    #[test]
    fn test_peer_registry_basics() {
        let mut registry = PeerRegistry::new();
        let p1 = Peer { id: None, ip: "1.1.1.1".to_string(), port: 80 };
        let p2 = Peer { id: None, ip: "2.2.2.2".to_string(), port: 80 };
        
        registry.add_peers(vec![p1.clone(), p2.clone()]);
        assert_eq!(registry.peer_count(), 2);
        
        let to_connect = registry.get_peer_to_connect().unwrap();
        assert!(to_connect.ip == "1.1.1.1" || to_connect.ip == "2.2.2.2");
        
        registry.update_state("1.1.1.1:80", ConnectionState::Connecting);
        let to_connect2 = registry.get_peer_to_connect().unwrap();
        assert_eq!(to_connect2.ip, "2.2.2.2");
    }
}
