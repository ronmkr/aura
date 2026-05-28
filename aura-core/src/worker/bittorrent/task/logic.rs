//! bt_task: Handles BitTorrent-specific task logic.

use crate::bitfield::Bitfield;
use crate::peer_registry::PeerRegistry;
use crate::piece_picker::PiecePicker;
use crate::task::TaskExtension;
use crate::torrent::Torrent;
use crate::tracker::Peer;
use crate::{Error, InfoHash, Result, TaskId};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

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

    pub async fn update_peer_interest(&self, addr: &str, interested: bool) {
        let mut registry = self.state.registry.lock().await;
        if let Some(ps) = registry.get_mut(addr) {
            ps.peer_interested = interested;
        }
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

impl TaskExtension for BtTask {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_arc(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}
