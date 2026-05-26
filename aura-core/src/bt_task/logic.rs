//! bt_task: Handles BitTorrent-specific task logic.

use crate::bitfield::Bitfield;
use crate::peer_registry::PeerRegistry;
use crate::piece_picker::PiecePicker;
use crate::torrent::Torrent;
use crate::tracker::{Peer, TrackerClient};
use crate::{Error, InfoHash, Result, TaskId};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};

#[derive(Debug)]
pub struct BtTaskState {
    pub info_hash: InfoHash,
    pub torrent: Mutex<Option<Torrent>>,
    pub bitfield: Mutex<Option<Bitfield>>,
    pub picker: Mutex<Option<PiecePicker>>,
    pub registry: Mutex<PeerRegistry>,
    pub sequential: std::sync::atomic::AtomicBool,
    pub db: sled::Db,
}

impl BtTaskState {
    pub fn new(torrent: Torrent, db: sled::Db) -> Self {
        let num_pieces = torrent.pieces_count();
        let info_hash = if let Some(h2) = torrent.info_hash_v2().unwrap_or(None) {
            InfoHash::V2(h2)
        } else {
            InfoHash::V1(torrent.info_hash_v1().unwrap_or(None).unwrap_or([0; 20]))
        };
        Self {
            info_hash,
            torrent: Mutex::new(Some(torrent)),
            bitfield: Mutex::new(Some(Bitfield::new(num_pieces))),
            picker: Mutex::new(Some(PiecePicker::new(num_pieces))),
            registry: Mutex::new(PeerRegistry::new()),
            sequential: std::sync::atomic::AtomicBool::new(false),
            db,
        }
    }

    pub fn new_magnet(info_hash: InfoHash, db: sled::Db) -> Self {
        Self {
            info_hash,
            torrent: Mutex::new(None),
            bitfield: Mutex::new(None),
            picker: Mutex::new(None),
            registry: Mutex::new(PeerRegistry::new()),
            sequential: std::sync::atomic::AtomicBool::new(false),
            db,
        }
    }

    pub async fn mature(&self, torrent: Torrent) {
        let num_pieces = torrent.pieces_count();

        // If v2, persist piece layers to DB
        if torrent.info.meta_version == Some(2) {
            if let Some(serde_bencode::value::Value::Dict(dict)) = &torrent.piece_layers {
                for (root, hashes) in dict {
                    if let serde_bencode::value::Value::Bytes(hash_bytes) = hashes {
                        let _ = self.db.insert(root, hash_bytes.clone());
                    }
                }
                let _ = self.db.flush();
            }
        }

        *self.torrent.lock().await = Some(torrent);
        *self.bitfield.lock().await = Some(Bitfield::new(num_pieces));
        *self.picker.lock().await = Some(PiecePicker::new(num_pieces));
    }
}

#[derive(Debug, Clone)]
pub struct BtTask {
    pub id: TaskId,
    pub state: Arc<BtTaskState>,
    pub dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
    pub lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
}

impl BtTask {
    pub async fn from_file(
        id: TaskId,
        path: &str,
        dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
        lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
        db: sled::Db,
    ) -> Result<Self> {
        let data = tokio::fs::read(path)
            .await
            .map_err(|e| Error::Protocol(format!("Failed to read torrent file: {}", e)))?;
        let torrent = Torrent::from_bytes(&data)?;
        Ok(Self {
            id,
            state: Arc::new(BtTaskState::new(torrent, db)),
            dht_tx,
            lpd_tx,
        })
    }

    pub fn from_magnet(
        id: TaskId,
        info_hash: InfoHash,
        dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
        lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
        db: sled::Db,
    ) -> Self {
        Self {
            id,
            state: Arc::new(BtTaskState::new_magnet(info_hash, db)),
            dht_tx,
            lpd_tx,
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
                            let ip: std::net::IpAddr = addr.ip();
                            peers.push(Peer {
                                id: None,
                                ip: ip.to_string(),
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

    pub async fn run_lpd_loop(
        &self,
        port: u16,
        token: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let info_hash = self.state.info_hash;
        info!(%self.id, "Starting LPD announcement loop");

        // Initial announcement
        let _ = self
            .lpd_tx
            .send(crate::lpd::LpdCommand::Announce { info_hash, port })
            .await;

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    let _ = self
                        .lpd_tx
                        .send(crate::lpd::LpdCommand::Remove {
                            info_hash,
                        })
                        .await;
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                }
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
        proxy: Option<String>,
    ) -> Result<()> {
        let tracker = TrackerClient::new(my_id, port, local_addr, user_agent, proxy);
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
        let mut registry = self.state.registry.lock().await;
        let peer = registry.get_peer_to_connect()?;
        let addr = format!("{}:{}", peer.ip, peer.port);

        // Try to pick a piece
        let bf_guard = self.state.bitfield.lock().await;
        let mut picker_guard = self.state.picker.lock().await;
        let sequential = self
            .state
            .sequential
            .load(std::sync::atomic::Ordering::Relaxed);

        let piece_idx = if let (Some(bf), Some(picker)) = (bf_guard.as_ref(), picker_guard.as_mut())
        {
            picker.pick_next(bf, &addr, sequential)
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

    pub async fn run_choker_loop(
        &self,
        worker_cmd_tx: tokio::sync::broadcast::Sender<crate::orchestrator::WorkerCommand>,
        token: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let mut ticks = 0;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                _ = interval.tick() => {}
            }
            if ticks == 0 {
                ticks += 1;
                continue; // Skip the immediate first tick
            }
            ticks += 1;
            let optimistic = ticks % 3 == 0;

            let mut registry = self.state.registry.lock().await;
            registry.tick_rates(10.0);

            let mut peers: Vec<_> = registry.get_all_connected().into_iter().collect();
            // Sort peers by download_rate descending
            peers.sort_by(|a, b| {
                b.download_rate
                    .partial_cmp(&a.download_rate)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut unchoked_count = 0;
            let mut optimistic_peer = None;

            for p in peers.iter_mut() {
                if unchoked_count < 4 {
                    // Top 4 peers (tit-for-tat)
                    if p.am_choking {
                        p.am_choking = false;
                        let _ = worker_cmd_tx.send(crate::orchestrator::WorkerCommand::Unchoke(
                            p.peer.ip.clone(),
                            p.peer.port,
                        ));
                    }
                    unchoked_count += 1;
                } else if optimistic && optimistic_peer.is_none() {
                    // Optimistic unchoke
                    if p.am_choking {
                        p.am_choking = false;
                        let _ = worker_cmd_tx.send(crate::orchestrator::WorkerCommand::Unchoke(
                            p.peer.ip.clone(),
                            p.peer.port,
                        ));
                    }
                    optimistic_peer = Some(p.peer.ip.clone());
                } else {
                    // Choke the rest
                    if !p.am_choking {
                        p.am_choking = true;
                        let _ = worker_cmd_tx.send(crate::orchestrator::WorkerCommand::Choke(
                            p.peer.ip.clone(),
                            p.peer.port,
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn run(
        &self,
        my_id: [u8; 20],
        _storage_tx: mpsc::Sender<crate::storage::StorageRequest>,
        _subtask_tx: mpsc::Sender<crate::orchestrator::SubTaskEvent>,
        token: tokio_util::sync::CancellationToken,
        _throttler: Arc<crate::throttler::Throttler>,
        worker_cmd_tx: tokio::sync::broadcast::Sender<crate::orchestrator::WorkerCommand>,
    ) -> Result<()> {
        let port = 6881; // TODO: make configurable
        let dht_token = token.clone();
        let lpd_token = token.clone();
        let tracker_token = token.clone();
        let choker_token = token.clone();

        let this = Arc::new(self.clone());
        let this_dht = this.clone();
        let this_lpd = this.clone();
        let this_tracker = this.clone();
        let this_choker = this.clone();

        tokio::spawn(async move {
            let _ = this_dht.run_dht_loop(dht_token).await;
        });

        tokio::spawn(async move {
            let _ = this_lpd.run_lpd_loop(port, lpd_token).await;
        });

        tokio::spawn(async move {
            let _ = this_tracker
                .run_tracker_loop(my_id, port, tracker_token, None, None, None)
                .await;
        });

        tokio::spawn(async move {
            let _ = this_choker
                .run_choker_loop(worker_cmd_tx, choker_token)
                .await;
        });

        Ok(())
    }
}
