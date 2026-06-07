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
    pub uploaded_bytes: u64,
    pub last_uploaded_bytes: u64,
    pub upload_rate: f64,
    pub is_optimistic_unchoke: bool,
    pub last_activity: std::time::Instant,
    pub error_count: u32,
}

#[derive(Debug)]
pub struct PeerRegistry {
    peers: HashMap<String, PeerState>, // Key: ip:port
    scorer: Box<dyn super::scorer::PeerScorer>,
    pub eviction_threshold: usize,
    pub eviction_percent: f32,
}

impl PeerRegistry {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
            scorer: Box::new(super::scorer::DefaultScorer),
            eviction_threshold: 500,
            eviction_percent: 0.1,
        }
    }

    pub fn with_scorer(scorer: Box<dyn super::scorer::PeerScorer>) -> Self {
        Self {
            peers: HashMap::new(),
            scorer,
            eviction_threshold: 500,
            eviction_percent: 0.1,
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
                    uploaded_bytes: 0,
                    last_uploaded_bytes: 0,
                    upload_rate: 0.0,
                    is_optimistic_unchoke: false,
                    last_activity: std::time::Instant::now(),
                    error_count: 0,
                }
            });
        }

        // Eviction logic
        if self.peers.len() > self.eviction_threshold {
            let mut all_peers: Vec<_> = self
                .peers
                .iter()
                .map(|(addr, ps)| (addr.clone(), self.scorer.calculate_score(ps)))
                .collect();
            all_peers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

            let to_evict = (all_peers.len() as f32 * self.eviction_percent) as usize;
            for (addr, _) in all_peers.iter().take(to_evict) {
                self.peers.remove(addr);
            }
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

    pub fn get_mut(&mut self, addr: &str) -> Option<&mut PeerState> {
        self.peers.get_mut(addr)
    }

    pub fn add_downloaded(&mut self, addr: &str, bytes: u64) {
        if let Some(ps) = self.peers.get_mut(addr) {
            ps.downloaded_bytes += bytes;
        }
    }

    pub fn add_uploaded(&mut self, addr: &str, bytes: u64) {
        if let Some(ps) = self.peers.get_mut(addr) {
            ps.uploaded_bytes += bytes;
        }
    }

    pub fn tick_rates(&mut self, elapsed_secs: f64) {
        for ps in self.peers.values_mut() {
            let d_bytes = ps.downloaded_bytes.saturating_sub(ps.last_downloaded_bytes);
            ps.download_rate = d_bytes as f64 / elapsed_secs;
            ps.last_downloaded_bytes = ps.downloaded_bytes;

            let u_bytes = ps.uploaded_bytes.saturating_sub(ps.last_uploaded_bytes);
            ps.upload_rate = u_bytes as f64 / elapsed_secs;
            ps.last_uploaded_bytes = ps.uploaded_bytes;
        }
    }

    pub fn reset_optimistic_unchokes(&mut self) {
        for ps in self.peers.values_mut() {
            ps.is_optimistic_unchoke = false;
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

    pub fn get_connected_peers(&self) -> Vec<Peer> {
        self.peers
            .values()
            .filter(|ps| {
                ps.state == ConnectionState::Handshaked || ps.state == ConnectionState::Connected
            })
            .map(|ps| ps.peer.clone())
            .collect()
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn record_error(&mut self, addr: &str) {
        if let Some(ps) = self.peers.get_mut(addr) {
            ps.error_count = ps.error_count.saturating_add(1);
            ps.last_activity = std::time::Instant::now();
        }
    }

    pub fn record_activity(&mut self, addr: &str) {
        if let Some(ps) = self.peers.get_mut(addr) {
            ps.last_activity = std::time::Instant::now();
        }
    }
}

impl Default for PeerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
