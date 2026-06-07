//! bt_task: Handles BitTorrent-specific task logic.

use crate::bitfield::Bitfield;
use crate::peer_registry::PeerRegistry;
use crate::piece_picker::PiecePicker;
use crate::task::TaskExtension;
use crate::torrent::Torrent;
use crate::tracker::Peer;
use crate::{Error, InfoHash, Result, TaskId, TenantId};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub struct BtTaskState {
    pub info_hash: InfoHash,
    pub torrent: Mutex<Option<Torrent>>,
    pub bitfield: Mutex<Option<Bitfield>>,
    pub picker: Mutex<Option<PiecePicker>>,
    pub registry: Mutex<PeerRegistry>,
    pub sequential: std::sync::atomic::AtomicBool,
    pub db: sled::Db,
    pub pieces_count: std::sync::atomic::AtomicUsize,
    pub resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
    pub tenant_id: Option<TenantId>,
    pub generations: Mutex<std::collections::HashMap<usize, u64>>,
    pub config: Arc<arc_swap::ArcSwap<crate::Config>>,
}

impl std::fmt::Debug for BtTaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BtTaskState")
            .field("info_hash", &self.info_hash)
            .field("tenant_id", &self.tenant_id)
            .finish()
    }
}

impl BtTaskState {
    pub fn new(
        torrent: Torrent,
        db: sled::Db,
        bitfield: Option<Bitfield>,
        resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
        tenant_id: Option<TenantId>,
        config: Arc<arc_swap::ArcSwap<crate::Config>>,
        selected_files: Option<&[bool]>,
    ) -> Self {
        let num_pieces = torrent.pieces_count();
        let info_hash = if let Some(h2) = torrent.info_hash_v2().unwrap_or(None) {
            InfoHash::V2(h2)
        } else {
            InfoHash::V1(torrent.info_hash_v1().unwrap_or(None).unwrap_or([0; 20]))
        };

        let bf = bitfield.unwrap_or_else(|| Bitfield::new(num_pieces));

        let picker = if let Some(selection) = selected_files {
            let selected_pieces = torrent.compute_selected_pieces(selection);
            PiecePicker::with_selection(num_pieces, selected_pieces)
        } else {
            PiecePicker::new(num_pieces)
        };

        Self {
            info_hash,
            torrent: Mutex::new(Some(torrent)),
            bitfield: Mutex::new(Some(bf)),
            picker: Mutex::new(Some(picker)),
            registry: Mutex::new(PeerRegistry::new()),
            sequential: std::sync::atomic::AtomicBool::new(false),
            db,
            pieces_count: std::sync::atomic::AtomicUsize::new(num_pieces),
            resource_governor,
            tenant_id,
            generations: Mutex::new(std::collections::HashMap::new()),
            config,
        }
    }

    pub fn new_magnet(
        info_hash: InfoHash,
        db: sled::Db,
        resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
        tenant_id: Option<TenantId>,
        config: Arc<arc_swap::ArcSwap<crate::Config>>,
    ) -> Self {
        Self {
            info_hash,
            torrent: Mutex::new(None),
            bitfield: Mutex::new(None),
            picker: Mutex::new(None),
            registry: Mutex::new(PeerRegistry::new()),
            sequential: std::sync::atomic::AtomicBool::new(false),
            db,
            pieces_count: std::sync::atomic::AtomicUsize::new(0),
            resource_governor,
            tenant_id,
            generations: Mutex::new(std::collections::HashMap::new()),
            config,
        }
    }

    pub async fn mature(&self, torrent: Torrent) {
        let num_pieces = torrent.pieces_count();
        self.pieces_count
            .store(num_pieces, std::sync::atomic::Ordering::Relaxed);

        // If v2, persist piece layers to DB
        if torrent.info.meta_version == Some(2) {
            let layer_index = torrent.get_piece_layer_index();
            if let Some(serde_bencode::value::Value::Dict(dict)) = &torrent.piece_layers {
                for (root, hashes) in dict {
                    if let serde_bencode::value::Value::Bytes(hash_bytes) = hashes {
                        // 1. Store under composite key (pieces_root + index)
                        let mut key = Vec::with_capacity(36);
                        key.extend_from_slice(root);
                        key.extend_from_slice(&layer_index.to_be_bytes());
                        let _ = self.db.insert(key, hash_bytes.clone());

                        // 2. Fallback to legacy key (pieces_root only)
                        let _ = self.db.insert(root, hash_bytes.clone());
                    }
                }
                let _ = self.db.flush();
            }
        }

        *self.torrent.lock().await = Some(torrent);
        let mut bf_guard = self.bitfield.lock().await;
        if bf_guard.is_none() || bf_guard.as_ref().map(|bf| bf.len()).unwrap_or(0) != num_pieces {
            *bf_guard = Some(Bitfield::new(num_pieces));
        }
        let bf_clone = bf_guard.as_ref().unwrap().clone();
        drop(bf_guard);

        let mut picker_guard = self.picker.lock().await;
        let mut picker = PiecePicker::new(num_pieces);
        // Synchronize picker with the preserved bitfield
        for i in 0..num_pieces {
            if bf_clone.get(i) {
                picker.mark_completed(i);
            }
        }
        *picker_guard = Some(picker);
        drop(picker_guard);
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
    #[allow(clippy::too_many_arguments)]
    pub async fn from_file(
        id: TaskId,
        path: &str,
        dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
        lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
        db: sled::Db,
        bitfield: Option<Bitfield>,
        resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
        tenant_id: Option<TenantId>,
        config: Arc<arc_swap::ArcSwap<crate::Config>>,
        selected_files: Option<&[bool]>,
    ) -> Result<Self> {
        let data = tokio::fs::read(path)
            .await
            .map_err(|e| Error::Protocol(format!("Failed to read torrent file: {}", e)))?;
        let torrent = Torrent::from_bytes(&data)?;
        Ok(Self {
            id,
            state: Arc::new(BtTaskState::new(
                torrent,
                db,
                bitfield,
                resource_governor,
                tenant_id,
                config,
                selected_files,
            )),
            dht_tx,
            lpd_tx,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_magnet(
        id: TaskId,
        info_hash: InfoHash,
        dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
        lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
        db: sled::Db,
        resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
        tenant_id: Option<TenantId>,
        config: Arc<arc_swap::ArcSwap<crate::Config>>,
    ) -> Self {
        Self {
            id,
            state: Arc::new(BtTaskState::new_magnet(
                info_hash,
                db,
                resource_governor,
                tenant_id,
                config,
            )),
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
        subtask_tx: mpsc::Sender<crate::orchestrator::SubTaskEvent>,
        token: tokio_util::sync::CancellationToken,
        _throttler: Arc<crate::throttler::Throttler>,
        worker_cmd_tx: tokio::sync::broadcast::Sender<crate::orchestrator::WorkerCommand>,
    ) -> Result<()> {
        let config = self.state.config.load();
        let port = config.network.listen_port;
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
                .run_tracker_loop(crate::worker::bittorrent::task::tracker::TrackerLoopArgs {
                    my_id,
                    port,
                    token: tracker_token,
                    local_addr: None,
                    user_agent: None,
                    proxy: None,
                    subtask_tx,
                })
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
