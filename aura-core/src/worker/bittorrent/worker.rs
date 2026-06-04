use crate::orchestrator::{SubTaskEvent, WorkerCommand};
use crate::storage::StorageRequest;
use crate::worker::bittorrent::handlers::PeerHandlerContext;
use crate::worker::bittorrent::task::BtTask;
use crate::{Error, InfoHash, Result, TaskId};
use bytes::BytesMut;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_util::codec::Framed;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

pub use super::protocol::{
    ExtendedHandshake, Handshake, MetadataMessage, PeerCodec, PeerId, PeerMessage, BLOCK_SIZE,
    HANDSHAKE_LEN,
};

/// Options for creating a new BitTorrent worker.
pub struct BtWorkerOptions {
    pub peer_addr: String,
    pub info_hash: InfoHash,
    pub peer_id: [u8; 20],
    pub my_id: [u8; 20],
    pub proxy: Option<String>,
    pub throttler: Arc<crate::throttler::Throttler>,
    pub pex_enabled: bool,
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
            pipeline_size: 10,
            metadata_buffer: None,
            ut_metadata_id: None,
            proxy: options.proxy,
            throttler: options.throttler,
            ut_pex_id: None,
            pex_enabled: options.pex_enabled,
            last_sent_pex_peers: std::collections::HashSet::new(),
            requested_hashes: std::collections::HashSet::new(),
        }
    }

    async fn connect_and_handshake(&self) -> Result<(TcpStream, [u8; 20], bool)> {
        debug!(addr = %self.peer_addr, "Connecting to peer...");
        let remote_addr: std::net::SocketAddr = self.peer_addr.parse().map_err(|e| {
            Error::Protocol(format!("Invalid peer address {}: {}", self.peer_addr, e))
        })?;

        let mut stream = timeout(
            std::time::Duration::from_secs(5),
            crate::net_util::connect_tcp_bound(
                remote_addr,
                None,
                self.local_addr,
                self.proxy.as_deref(),
            ),
        )
        .await
        .map_err(|_| Error::Protocol("Peer connection timeout".to_string()))??;

        debug!(addr = %self.peer_addr, "Sending handshake...");
        let handshake = Handshake::new(self.info_hash.for_handshake(), self.my_id);

        timeout(std::time::Duration::from_secs(5), async {
            stream.write_all(&handshake.serialize()).await?;
            let mut buf = [0u8; HANDSHAKE_LEN];
            use tokio::io::AsyncReadExt;
            stream.read_exact(&mut buf).await?;
            Ok::<[u8; HANDSHAKE_LEN], std::io::Error>(buf)
        })
        .await
        .map_err(|_| Error::Protocol("Peer handshake timeout".to_string()))?
        .map_err(|e| Error::Protocol(format!("Peer handshake error: {}", e)))
        .and_then(|buf| {
            let res_handshake = Handshake::deserialize(&buf)?;

            if res_handshake.info_hash != self.info_hash.for_handshake() {
                return Err(Error::Protocol("Handshake info_hash mismatch".to_string()));
            }

            Ok((
                stream,
                res_handshake.peer_id,
                res_handshake.extension_protocol,
            ))
        })
    }

    pub async fn run_loop(&mut self, args: BtWorkerArgs) -> Result<()> {
        let res = self.connect_and_handshake().await;
        let (stream, peer_id, ext_support) = match res {
            Ok(val) => val,
            Err(e) => {
                args.task
                    .update_peer_state(
                        &self.peer_addr,
                        crate::peer_registry::ConnectionState::Disconnected,
                    )
                    .await;
                return Err(e);
            }
        };
        self.peer_id = peer_id;
        info!(addr = %self.peer_addr, "Handshake completed successfully");

        let peer_addr = self.peer_addr.clone();
        let task = args.task.clone();
        let res = self
            .run_loop_with_stream_and_ext(stream, args, ext_support)
            .await;

        task.update_peer_state(
            &peer_addr,
            crate::peer_registry::ConnectionState::Disconnected,
        )
        .await;
        res
    }

    pub async fn run_loop_with_stream(
        &mut self,
        stream: TcpStream,
        args: BtWorkerArgs,
    ) -> Result<()> {
        self.run_loop_with_stream_and_ext(stream, args, false).await
    }

    async fn run_loop_with_stream_and_ext(
        &mut self,
        stream: TcpStream,
        args: BtWorkerArgs,
        ext_support: bool,
    ) -> Result<()> {
        let BtWorkerArgs {
            meta_id,
            sub_id,
            task,
            storage_tx,
            subtask_tx,
            mut command_rx,
            token,
        } = args;

        let mut framed = Framed::new(stream, PeerCodec);

        if ext_support {
            let mut m = std::collections::HashMap::new();
            m.insert("ut_metadata".to_string(), 1);
            if self.pex_enabled {
                m.insert("ut_pex".to_string(), 2);
            }
            let ext_hs = ExtendedHandshake {
                m: Some(m),
                metadata_size: None,
            };
            let payload = serde_bencode::to_bytes(&ext_hs).map_err(|e| {
                Error::Protocol(format!("Failed to encode extended handshake: {}", e))
            })?;
            framed
                .send(PeerMessage::Extended {
                    id: 0,
                    payload: payload.into(),
                })
                .await?;
        }

        framed.send(PeerMessage::Interested).await?;

        let mut peer_choking = true;
        let peer_addr = self.peer_addr.clone();

        info!(addr = %peer_addr, "Entering main peer message loop");

        loop {
            let mut ctx = PeerHandlerContext {
                framed: &mut framed,
                task: &task,
                meta_id,
                sub_id,
                storage_tx: storage_tx.clone(),
                subtask_tx: subtask_tx.clone(),
                peer_choking: &mut peer_choking,
            };

            tokio::select! {
                _ = token.cancelled() => break,
                Ok(cmd) = command_rx.recv() => {
                    match cmd {
                        WorkerCommand::CancelPiece(piece_idx) => {
                            if Some(piece_idx) == self.current_piece {
                                debug!(addr = %peer_addr, %piece_idx, "Received cancellation for current piece");
                                if let Some(ref mut guard) = self.active_guard {
                                    guard.complete();
                                }
                                self.active_guard = None;
                                self.current_piece = None;
                                self.is_endgame = false;
                                self.bytes_received = 0;
                                self.bytes_requested = 0;
                                self.piece_buffer.clear();
                                self.trigger_request(&mut ctx).await?;
                            }
                        }
                        WorkerCommand::RequestPiece(piece_idx) => {
                            if self.current_piece.is_none() {
                                debug!(addr = %peer_addr, %piece_idx, "Received forced piece request (Endgame)");
                                self.current_piece = Some(piece_idx);
                                self.is_endgame = false;
                                self.bytes_received = 0;
                                self.bytes_requested = 0;
                                let state_clone = task.state.clone();
                                let guard = crate::piece_picker::PieceGuard::new(piece_idx, move |idx| {
                                    tokio::spawn(async move {
                                        if let Some(picker) = state_clone.picker.lock().await.as_mut() {
                                            picker.release_piece(idx);
                                        }
                                    });
                                });
                                self.active_guard = Some(guard);
                                self.trigger_request(&mut ctx).await?;
                            }
                        }
                        WorkerCommand::EndgameFetch(piece_idx) => {
                            if self.current_piece.is_none() {
                                debug!(addr = %peer_addr, %piece_idx, "Received forced piece request (EndgameFetch)");
                                self.current_piece = Some(piece_idx);
                                self.is_endgame = true;
                                self.bytes_received = 0;
                                self.bytes_requested = 0;
                                let state_clone = task.state.clone();
                                let guard = crate::piece_picker::PieceGuard::new(piece_idx, move |idx| {
                                    tokio::spawn(async move {
                                        if let Some(picker) = state_clone.picker.lock().await.as_mut() {
                                            picker.release_piece(idx);
                                        }
                                    });
                                });
                                self.active_guard = Some(guard);
                                self.trigger_request(&mut ctx).await?;
                            } else if self.current_piece == Some(piece_idx) && !self.is_endgame {
                                debug!(addr = %peer_addr, %piece_idx, "Promoting current piece request to EndgameFetch");
                                self.is_endgame = true;
                                self.trigger_request(&mut ctx).await?;
                            }
                        }
                        WorkerCommand::CheckWork => {
                            if self.current_piece.is_none() && !*ctx.peer_choking {
                                self.trigger_request(&mut ctx).await?;
                            }
                        }
                        WorkerCommand::Choke(addr, _) => {
                            if addr == self.peer_addr {
                                debug!(addr = %peer_addr, "Choking peer based on tit-for-tat");
                                let _ = ctx.framed.send(PeerMessage::Choke).await;
                            }
                        }
                        WorkerCommand::Unchoke(addr, _) => {
                            if addr == self.peer_addr {
                                debug!(addr = %peer_addr, "Unchoking peer based on tit-for-tat");
                                let _ = ctx.framed.send(PeerMessage::Unchoke).await;
                            }
                        }
                        WorkerCommand::PexUpdate(active_peers) => {
                            if !self.pex_enabled {
                                continue;
                            }
                            if let Some(pex_id) = self.ut_pex_id {
                                let added: Vec<_> = active_peers
                                    .difference(&self.last_sent_pex_peers)
                                    .copied()
                                    .collect();
                                let dropped: Vec<_> = self
                                    .last_sent_pex_peers
                                    .difference(&active_peers)
                                    .copied()
                                    .collect();

                                if !added.is_empty() || !dropped.is_empty() {
                                    let pex_msg = crate::worker::bittorrent::protocol::PexMessage::encode_peers(&added, &dropped);
                                    if let Ok(payload) = serde_bencode::to_bytes(&pex_msg) {
                                        let _ = ctx.framed.send(PeerMessage::Extended {
                                            id: pex_id,
                                            payload: payload.into(),
                                        }).await;
                                    }
                                    self.last_sent_pex_peers = active_peers;
                                }
                            }
                        }
                    }
                }
                msg_res = ctx.framed.next() => {
                    match msg_res {
                        Some(Ok(msg)) => {
                            self.handle_peer_message(msg, ctx).await?;
                        }
                        Some(Err(e)) => return Err(e),
                        None => break,
                    }
                },
            }
        }
        Ok(())
    }
}
