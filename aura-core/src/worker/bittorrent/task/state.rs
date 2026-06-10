//! bt_task_state: Handles BitTorrent task state management.

use crate::bitfield::Bitfield;
use crate::peer_registry::PeerRegistry;
use crate::piece_picker::PiecePicker;
use crate::torrent::Torrent;
use crate::{InfoHash, TenantId};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct BtTaskState {
    pub info_hash: InfoHash,
    pub torrent: Mutex<Option<Torrent>>,
    pub bitfield: Mutex<Option<Bitfield>>,
    pub picker: Mutex<Option<PiecePicker>>,
    pub registry: Mutex<PeerRegistry>,
    pub sequential: std::sync::atomic::AtomicBool,
    pub streaming_mode: std::sync::atomic::AtomicBool,
    pub db: sled::Db,
    pub pieces_count: std::sync::atomic::AtomicUsize,
    pub resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
    pub tenant_id: Option<TenantId>,
    pub generations: Mutex<std::collections::HashMap<usize, u64>>,
    pub config: Arc<arc_swap::ArcSwap<crate::Config>>,
    pub uploaded_length: std::sync::atomic::AtomicU64,
    pub seeding_start_time: std::sync::Mutex<Option<chrono::DateTime<chrono::Utc>>>,
    pub seed_ratio: std::sync::Mutex<Option<f32>>,
    pub seed_time: std::sync::Mutex<Option<u32>>,
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
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        torrent: Torrent,
        db: sled::Db,
        bitfield: Option<Bitfield>,
        resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
        tenant_id: Option<TenantId>,
        config: Arc<arc_swap::ArcSwap<crate::Config>>,
        selected_files: Option<&[bool]>,
        streaming_mode: bool,
    ) -> Self {
        let num_pieces = torrent.pieces_count();
        let info_hash = if let Some(h2) = torrent.info_hash_v2().unwrap_or(None) {
            InfoHash::V2(h2)
        } else {
            InfoHash::V1(torrent.info_hash_v1().unwrap_or(None).unwrap_or([0; 20]))
        };
        let bf = bitfield.unwrap_or_else(|| Bitfield::new(num_pieces));

        let bt_config = config.load().bittorrent.clone();

        let picker = if let Some(selection) = selected_files {
            let selected_pieces = torrent.compute_selected_pieces(selection);
            PiecePicker::with_selection(
                num_pieces,
                selected_pieces,
                bt_config.endgame_threshold_pieces,
                bt_config.endgame_threshold_percent,
                bt_config.streaming_metadata_pieces,
            )
        } else {
            let mut p = PiecePicker::new(num_pieces);
            p.endgame_threshold_pieces = bt_config.endgame_threshold_pieces;
            p.endgame_threshold_percent = bt_config.endgame_threshold_percent;
            p.streaming_metadata_pieces = bt_config.streaming_metadata_pieces;
            p
        };

        let mut registry = PeerRegistry::new();
        registry.eviction_threshold = bt_config.peer_eviction_threshold;
        registry.eviction_percent = bt_config.peer_eviction_percent;

        Self {
            info_hash,
            torrent: Mutex::new(Some(torrent)),
            bitfield: Mutex::new(Some(bf)),
            picker: Mutex::new(Some(picker)),
            registry: Mutex::new(registry),
            sequential: std::sync::atomic::AtomicBool::new(false),
            streaming_mode: std::sync::atomic::AtomicBool::new(streaming_mode),
            db,
            pieces_count: std::sync::atomic::AtomicUsize::new(num_pieces),
            resource_governor,
            tenant_id,
            generations: Mutex::new(std::collections::HashMap::new()),
            config,
            uploaded_length: std::sync::atomic::AtomicU64::new(0),
            seeding_start_time: std::sync::Mutex::new(None),
            seed_ratio: std::sync::Mutex::new(None),
            seed_time: std::sync::Mutex::new(None),
        }
    }

    pub fn new_magnet(
        info_hash: InfoHash,
        db: sled::Db,
        resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
        tenant_id: Option<TenantId>,
        config: Arc<arc_swap::ArcSwap<crate::Config>>,
        streaming_mode: bool,
    ) -> Self {
        Self {
            info_hash,
            torrent: Mutex::new(None),
            bitfield: Mutex::new(None),
            picker: Mutex::new(None),
            registry: Mutex::new(PeerRegistry::new()),
            sequential: std::sync::atomic::AtomicBool::new(false),
            streaming_mode: std::sync::atomic::AtomicBool::new(streaming_mode),
            db,
            pieces_count: std::sync::atomic::AtomicUsize::new(0),
            resource_governor,
            tenant_id,
            generations: Mutex::new(std::collections::HashMap::new()),
            config,
            uploaded_length: std::sync::atomic::AtomicU64::new(0),
            seeding_start_time: std::sync::Mutex::new(None),
            seed_ratio: std::sync::Mutex::new(None),
            seed_time: std::sync::Mutex::new(None),
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
