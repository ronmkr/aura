//! bt_task: Handles BitTorrent-specific task logic.

use crate::bitfield::Bitfield;
use crate::task::extension::TaskExtension;
use crate::torrent::Torrent;
use crate::tracker::Peer;
use crate::{Error, InfoHash, Result, TaskId, TenantId};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::state::BtTaskState;

#[derive(Debug, Clone)]
pub struct BtTask {
    pub id: TaskId,
    pub state: Arc<BtTaskState>,
    pub dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
    pub lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
}

pub struct BtTaskFromFileArgs<'a> {
    pub id: TaskId,
    pub path: &'a str,
    pub dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
    pub lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
    pub db: sled::Db,
    pub bitfield: Option<Bitfield>,
    pub resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
    pub tenant_id: Option<TenantId>,
    pub config: Arc<arc_swap::ArcSwap<crate::Config>>,
    pub selected_files: Option<&'a [bool]>,
    pub streaming_mode: bool,
}

pub struct BtTaskFromMagnetArgs {
    pub id: TaskId,
    pub info_hash: InfoHash,
    pub dht_tx: mpsc::Sender<crate::dht::DhtCommand>,
    pub lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
    pub db: sled::Db,
    pub resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
    pub tenant_id: Option<TenantId>,
    pub config: Arc<arc_swap::ArcSwap<crate::Config>>,
    pub streaming_mode: bool,
}

impl BtTask {
    pub async fn from_file(args: BtTaskFromFileArgs<'_>) -> Result<Self> {
        let data = tokio::fs::read(args.path)
            .await
            .map_err(|e| Error::Protocol(format!("Failed to read torrent file: {}", e)))?;
        let torrent = Torrent::from_bytes(&data)?;
        Ok(Self {
            id: args.id,
            state: Arc::new(BtTaskState::new(
                torrent,
                args.db,
                args.bitfield,
                args.resource_governor,
                args.tenant_id,
                args.config,
                args.selected_files,
                args.streaming_mode,
            )),
            dht_tx: args.dht_tx,
            lpd_tx: args.lpd_tx,
        })
    }

    pub fn from_magnet(args: BtTaskFromMagnetArgs) -> Self {
        Self {
            id: args.id,
            state: Arc::new(BtTaskState::new_magnet(
                args.info_hash,
                args.db,
                args.resource_governor,
                args.tenant_id,
                args.config,
                args.streaming_mode,
            )),
            dht_tx: args.dht_tx,
            lpd_tx: args.lpd_tx,
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
        let streaming = self
            .state
            .streaming_mode
            .load(std::sync::atomic::Ordering::Relaxed);

        let piece_idx = if let (Some(bf), Some(picker)) = (bf_guard.as_ref(), picker_guard.as_mut())
        {
            picker.pick_next(bf, &addr, sequential, streaming)
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
        _storage_client: Arc<dyn crate::storage::StorageDispatch>,
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
