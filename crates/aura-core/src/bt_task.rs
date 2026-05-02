//! bt_task: Handles BitTorrent-specific task logic.

use crate::bitfield::Bitfield;
use crate::peer_registry::PeerRegistry;
use crate::piece_picker::PiecePicker;
use crate::torrent::Torrent;
use crate::tracker::{Peer, TrackerClient};
use crate::{Error, Result, TaskId};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct BtTaskState {
    pub info_hash: [u8; 20],
    pub torrent: Mutex<Option<Torrent>>,
    pub bitfield: Mutex<Option<Bitfield>>,
    pub picker: Mutex<Option<PiecePicker>>,
    pub registry: Mutex<PeerRegistry>,
}

impl BtTaskState {
    pub fn new(torrent: Torrent) -> Self {
        let num_pieces = torrent.pieces_count();
        let info_hash = torrent.info_hash().unwrap_or([0; 20]);
        Self {
            info_hash,
            torrent: Mutex::new(Some(torrent)),
            bitfield: Mutex::new(Some(Bitfield::new(num_pieces))),
            picker: Mutex::new(Some(PiecePicker::new(num_pieces))),
            registry: Mutex::new(PeerRegistry::new()),
        }
    }

    pub fn new_magnet(info_hash: [u8; 20]) -> Self {
        Self {
            info_hash,
            torrent: Mutex::new(None),
            bitfield: Mutex::new(None),
            picker: Mutex::new(None),
            registry: Mutex::new(PeerRegistry::new()),
        }
    }

    pub async fn mature(&self, torrent: Torrent) {
        let num_pieces = torrent.pieces_count();
        *self.torrent.lock().await = Some(torrent);
        *self.bitfield.lock().await = Some(Bitfield::new(num_pieces));
        *self.picker.lock().await = Some(PiecePicker::new(num_pieces));
    }
}

#[derive(Debug)]
pub struct BtTask {
    pub id: TaskId,
    pub state: Arc<BtTaskState>,
    pub dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
}

impl BtTask {
    pub async fn from_file(
        id: TaskId,
        path: &str,
        dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
    ) -> Result<Self> {
        let data = tokio::fs::read(path)
            .await
            .map_err(|e| Error::Protocol(format!("Failed to read torrent file: {}", e)))?;
        let torrent = Torrent::from_bytes(&data)?;
        Ok(Self {
            id,
            state: Arc::new(BtTaskState::new(torrent)),
            dht_tx,
        })
    }

    pub fn from_magnet(
        id: TaskId,
        info_hash: [u8; 20],
        dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
    ) -> Self {
        Self {
            id,
            state: Arc::new(BtTaskState::new_magnet(info_hash)),
            dht_tx,
        }
    }

    pub async fn run_dht_loop(&self, token: tokio_util::sync::CancellationToken) -> Result<()> {
        let info_hash = self.state.info_hash;
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
            let torrent_opt = self.state.torrent.lock().await.clone();
            if let Some(torrent) = torrent_opt {
                tokio::select! {
                    _ = token.cancelled() => break,
                    res = tracker.announce(&torrent) => {
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
            } else {
                // If we don't have a torrent yet, we can't announce to trackers (mostly)
                // Actually some trackers allow info_hash only, but our TrackerClient uses torrent.
                // For now, we rely on DHT for magnet links.
            }

            tokio::select! {
                _ = token.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
            }
        }
        Ok(())
    }

    /// Picks a peer to connect to and optionally a piece to download.
    pub async fn pick_work(&self) -> Option<(Option<usize>, Peer)> {
        let registry = self.state.registry.lock().await;
        let peer = registry.get_peer_to_connect()?;
        let addr = format!("{}:{}", peer.ip, peer.port);

        // Try to pick a piece
        let bf_guard = self.state.bitfield.lock().await;
        let picker_guard = self.state.picker.lock().await;

        let piece_idx = if let (Some(bf), Some(picker)) = (bf_guard.as_ref(), picker_guard.as_ref())
        {
            picker.pick_next(bf, &addr)
        } else {
            None
        };

        Some((piece_idx, peer))
    }

    pub async fn update_peer_state(
        &self,
        addr: &str,
        state: crate::peer_registry::ConnectionState,
    ) {
        let mut registry = self.state.registry.lock().await;
        registry.update_state(addr, state);
    }
}
