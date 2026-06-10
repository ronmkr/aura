use crate::orchestrator::{SubTaskEvent, WorkerCommand};
use crate::storage::StorageRequest;
use crate::worker::bittorrent::task::BtTask;
use crate::{InfoHash, TaskId};
use bytes::BytesMut;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

/// Options for creating a new BitTorrent worker.
pub struct BtWorkerOptions {
    pub peer_addr: String,
    pub info_hash: InfoHash,
    pub peer_id: [u8; 20],
    pub my_id: [u8; 20],
    pub proxy: Option<String>,
    pub throttler: Arc<crate::throttler::Throttler>,
    pub pex_enabled: bool,
    pub pipeline_size: usize,
    pub connect_timeout_secs: u64,
    pub happy_eyeballs_stagger_ms: u64,
}

/// Arguments for the BitTorrent worker main loop.
pub struct BtWorkerArgs {
    pub meta_id: TaskId,
    pub sub_id: TaskId,
    pub task: Arc<BtTask>,
    pub storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
    pub subtask_tx: tokio::sync::mpsc::Sender<SubTaskEvent>,
    pub command_rx: tokio::sync::broadcast::Receiver<WorkerCommand>,
    pub token: CancellationToken,
}

pub struct BtWorker {
    pub peer_addr: String,
    pub info_hash: InfoHash,
    pub peer_id: [u8; 20],
    pub my_id: [u8; 20],
    pub current_piece: Option<usize>,
    pub is_endgame: bool,
    pub active_guard: Option<crate::piece_picker::PieceGuard>,
    pub bytes_received: u64,
    pub bytes_requested: u64,
    pub piece_buffer: BytesMut,
    pub memory_guard: Option<crate::orchestrator::resource_governor::MemoryGuard>,
    pub current_generation: u64,
    pub local_addr: Option<std::net::IpAddr>,
    pub pipeline_size: usize,
    pub metadata_buffer: Option<BytesMut>,
    pub ut_metadata_id: Option<u8>,
    pub proxy: Option<String>,
    pub throttler: Arc<crate::throttler::Throttler>,
    pub ut_pex_id: Option<u8>,
    pub pex_enabled: bool,
    pub last_sent_pex_peers: std::collections::HashSet<std::net::SocketAddr>,
    pub requested_hashes: std::collections::HashSet<[u8; 32]>,
    pub connect_timeout_secs: u64,
    pub happy_eyeballs_stagger_ms: u64,
}

impl BtWorker {
    pub fn new(options: BtWorkerOptions) -> Self {
        Self {
            peer_addr: options.peer_addr,
            info_hash: options.info_hash,
            peer_id: options.peer_id,
            my_id: options.my_id,
            current_piece: None,
            is_endgame: false,
            active_guard: None,
            bytes_received: 0,
            bytes_requested: 0,
            piece_buffer: BytesMut::new(),
            memory_guard: None,
            current_generation: 0,
            local_addr: None,
            pipeline_size: options.pipeline_size,
            metadata_buffer: None,
            ut_metadata_id: None,
            proxy: options.proxy,
            throttler: options.throttler,
            ut_pex_id: None,
            pex_enabled: options.pex_enabled,
            last_sent_pex_peers: std::collections::HashSet::new(),
            requested_hashes: std::collections::HashSet::new(),
            connect_timeout_secs: options.connect_timeout_secs,
            happy_eyeballs_stagger_ms: options.happy_eyeballs_stagger_ms,
        }
    }
}
