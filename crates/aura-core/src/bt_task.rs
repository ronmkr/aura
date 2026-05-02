//! bt_task: Handles BitTorrent-specific task logic.

use crate::{Result, TaskId, Error};
use crate::torrent::Torrent;
use crate::bitfield::Bitfield;
use crate::piece_picker::PiecePicker;
use crate::peer_registry::PeerRegistry;
use crate::tracker::{TrackerClient, Peer};
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tracing::{info, debug, warn};

#[derive(Debug)]
pub struct BtTaskState {
    pub torrent: Torrent,
    pub bitfield: Mutex<Bitfield>,
    pub picker: Mutex<PiecePicker>,
    pub registry: Mutex<PeerRegistry>,
}

impl BtTaskState {
    pub fn new(torrent: Torrent) -> Self {
        let num_pieces = torrent.pieces_count();
        Self {
            torrent,
            bitfield: Mutex::new(Bitfield::new(num_pieces)),
            picker: Mutex::new(PiecePicker::new(num_pieces)),
            registry: Mutex::new(PeerRegistry::new()),
        }
    }
}

#[derive(Debug)]
pub struct BtTask {
    pub id: TaskId,
    pub state: Arc<BtTaskState>,
    pub dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
}

impl BtTask {
    pub async fn from_file(id: TaskId, path: &str, dht_tx: mpsc::Sender<crate::dht::DhtCommand>) -> Result<Self> {
        let data = tokio::fs::read(path).await
            .map_err(|e| Error::Protocol(format!("Failed to read torrent file: {}", e)))?;
        let torrent = Torrent::from_bytes(&data)?;
        Ok(Self {
            id,
            state: Arc::new(BtTaskState::new(torrent)),
            dht_tx,
        })
    }

    pub async fn run_dht_loop(&self, token: tokio_util::sync::CancellationToken) -> Result<()> {
        let info_hash = self.state.torrent.info_hash()?;
        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                _ = async {
                    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
                    let cmd = crate::dht::DhtCommand::GetPeers {
                        info_hash,
                        reply_tx: tx,
                    };

                    if let Err(e) = self.dht_tx.send(cmd).await {
                        warn!("Failed to send DHT command: {}", e);
                        return;
                    }

                    if let Some(addrs) = rx.recv().await {
                        let mut peers = Vec::new();
                        for addr in addrs {
                            peers.push(Peer {
                                id: None,
                                ip: addr.ip().to_string(),
                                port: addr.port(),
                            });
                        }
                        
                        if !peers.is_empty() {
                            info!(%self.id, count = peers.len(), "Discovered peers from DHT");
                            let mut registry = self.state.registry.lock().await;
                            let added = registry.add_peers(peers);
                            debug!(%self.id, added, "Added unique DHT peers to registry");
                        }
                    }
                } => {}
            }

            tokio::select! {
                _ = token.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(120)) => {}
            }
        }
        Ok(())
    }

    pub async fn run_tracker_loop(
        &self, 
        my_id: [u8; 20], 
        port: u16, 
        token: tokio_util::sync::CancellationToken,
        local_addr: Option<std::net::IpAddr>,
        user_agent: Option<String>,
    ) -> Result<()> {
        let tracker = TrackerClient::new(my_id, port, local_addr, user_agent);
        info!(%self.id, "Starting tracker announce");
        
        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                res = tracker.announce(&self.state.torrent) => {
                    match res {
                        Ok(peers) => {
                            info!(%self.id, count = peers.len(), "Discovered peers from tracker");
                            let mut registry = self.state.registry.lock().await;
                            let added = registry.add_peers(peers);
                            debug!(%self.id, added, "Added unique peers to registry");
                        }
                        Err(e) => {
                            tracing::warn!(%self.id, error = %e, "All tracker announces failed");
                        }
                    }
                }
            }
            
            tokio::select! {
                _ = token.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
            }
        }
        Ok(())
    }

    /// Picks a peer to connect to and optionally a piece to download.
    /// Initially, we connect to peers just to get their bitfields.
    pub async fn pick_work(&self) -> Option<(Option<usize>, Peer)> {
        let registry = self.state.registry.lock().await;
        let peer = registry.get_peer_to_connect()?;
        let addr = format!("{}:{}", peer.ip, peer.port);
        
        // Try to pick a piece, but it's okay if we can't yet (we still need bitfields)
        let bf = self.state.bitfield.lock().await;
        let picker = self.state.picker.lock().await;
        let piece_idx = picker.pick_next(&bf, &addr);
        
        Some((piece_idx, peer))
    }

    pub async fn update_peer_state(&self, addr: &str, state: crate::peer_registry::ConnectionState) {
        let mut registry = self.state.registry.lock().await;
        registry.update_state(addr, state);
    }
}
