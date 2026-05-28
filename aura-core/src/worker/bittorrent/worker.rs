use crate::worker::bittorrent::task::BtTask;
use crate::orchestrator::{SubTaskEvent, WorkerCommand};
use crate::storage::StorageRequest;
use crate::{Error, InfoHash, Result, TaskId};
use bytes::BytesMut;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

pub use super::protocol::{
    ExtendedHandshake, Handshake, MetadataMessage, PeerCodec, PeerId, PeerMessage, BLOCK_SIZE,
    HANDSHAKE_LEN,
};

pub struct BtWorker {
    pub peer_addr: String,
    pub info_hash: InfoHash,
    pub peer_id: [u8; 20],
    pub my_id: [u8; 20],
    pub current_piece: Option<usize>,
    pub bytes_received: u64,
    pub bytes_requested: u64,
    pub piece_buffer: BytesMut,
    pub local_addr: Option<std::net::IpAddr>,
    pub pipeline_size: usize,
    pub metadata_buffer: Option<BytesMut>,
    pub ut_metadata_id: Option<u8>,
    pub proxy: Option<String>,
    pub throttler: Arc<crate::throttler::Throttler>,
    pub ut_pex_id: Option<u8>,
    pub pex_enabled: bool,
    pub last_sent_pex_peers: std::collections::HashSet<std::net::SocketAddr>,
}

impl BtWorker {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        peer_addr: String,
        info_hash: InfoHash,
        peer_id: [u8; 20],
        my_id: [u8; 20],
        proxy: Option<String>,
        throttler: Arc<crate::throttler::Throttler>,
        pex_enabled: bool,
    ) -> Self {
        Self {
            peer_addr,
            info_hash,
            peer_id,
            my_id,
            current_piece: None,
            bytes_received: 0,
            bytes_requested: 0,
            piece_buffer: BytesMut::new(),
            local_addr: None,
            pipeline_size: 10,
            metadata_buffer: None,
            ut_metadata_id: None,
            proxy,
            throttler,
            ut_pex_id: None,
            pex_enabled,
            last_sent_pex_peers: std::collections::HashSet::new(),
        }
    }

    async fn connect_and_handshake(&self) -> Result<(TcpStream, [u8; 20], bool)> {
        debug!(addr = %self.peer_addr, "Connecting to peer...");
        let remote_addr: std::net::SocketAddr = self.peer_addr.parse().map_err(|e| {
            Error::Protocol(format!("Invalid peer address {}: {}", self.peer_addr, e))
        })?;

        let mut stream = tokio::time::timeout(
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

        tokio::time::timeout(std::time::Duration::from_secs(5), async {
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

    #[allow(clippy::too_many_arguments)]
    pub async fn run_loop(
        mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        task: Arc<BtTask>,
        storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
        subtask_tx: tokio::sync::mpsc::Sender<SubTaskEvent>,
        command_rx: tokio::sync::broadcast::Receiver<WorkerCommand>,
        token: CancellationToken,
    ) -> Result<()> {
        let (stream, peer_id, ext_support) = self.connect_and_handshake().await?;
        self.peer_id = peer_id;

        self.run_loop_with_stream_and_ext(
            stream,
            meta_id,
            sub_id,
            task,
            storage_tx,
            subtask_tx,
            command_rx,
            token,
            ext_support,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_loop_with_stream(
        self,
        stream: TcpStream,
        meta_id: TaskId,
        sub_id: TaskId,
        task: Arc<BtTask>,
        storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
        subtask_tx: tokio::sync::mpsc::Sender<SubTaskEvent>,
        command_rx: tokio::sync::broadcast::Receiver<WorkerCommand>,
        token: CancellationToken,
    ) -> Result<()> {
        self.run_loop_with_stream_and_ext(
            stream, meta_id, sub_id, task, storage_tx, subtask_tx, command_rx, token, false,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_loop_with_stream_and_ext(
        mut self,
        stream: TcpStream,
        meta_id: TaskId,
        sub_id: TaskId,
        task: Arc<BtTask>,
        storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
        subtask_tx: tokio::sync::mpsc::Sender<SubTaskEvent>,
        mut command_rx: tokio::sync::broadcast::Receiver<WorkerCommand>,
        token: CancellationToken,
        ext_support: bool,
    ) -> Result<()> {
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
            tokio::select! {
                _ = token.cancelled() => break,
                Ok(cmd) = command_rx.recv() => {
                    match cmd {
                        WorkerCommand::CancelPiece(piece_idx) => {
                            if Some(piece_idx) == self.current_piece {
                                debug!(addr = %peer_addr, %piece_idx, "Received cancellation for current piece");
                                self.current_piece = None;
                                self.bytes_received = 0;
                                self.bytes_requested = 0;
                                self.piece_buffer.clear();
                                self.trigger_request(&mut framed, &task, meta_id, sub_id, storage_tx.clone(), subtask_tx.clone()).await?;
                            }
                        }
                        WorkerCommand::RequestPiece(piece_idx) => {
                            if self.current_piece.is_none() {
                                debug!(addr = %peer_addr, %piece_idx, "Received forced piece request (Endgame)");
                                self.current_piece = Some(piece_idx);
                                self.bytes_received = 0;
                                self.bytes_requested = 0;
                                self.trigger_request(&mut framed, &task, meta_id, sub_id, storage_tx.clone(), subtask_tx.clone()).await?;
                            }
                        }
                        WorkerCommand::Choke(addr, _) => {
                            if addr == self.peer_addr {
                                debug!(addr = %peer_addr, "Choking peer based on tit-for-tat");
                                let _ = framed.send(PeerMessage::Choke).await;
                            }
                        }
                        WorkerCommand::Unchoke(addr, _) => {
                            if addr == self.peer_addr {
                                debug!(addr = %peer_addr, "Unchoking peer based on tit-for-tat");
                                let _ = framed.send(PeerMessage::Unchoke).await;
                            }
                        }
                        WorkerCommand::PexUpdate(active_peers) => {
                            if !self.pex_enabled {
                                continue;
                            }
                            if let Some(pex_id) = self.ut_pex_id {
                                let added: Vec<_> = active_peers.difference(&self.last_sent_pex_peers).copied().collect();
                                let dropped: Vec<_> = self.last_sent_pex_peers.difference(&active_peers).copied().collect();

                                if !added.is_empty() || !dropped.is_empty() {
                                    let pex_msg = crate::worker::bittorrent::protocol::PexMessage::encode_peers(&added, &dropped);
                                    if let Ok(payload) = serde_bencode::to_bytes(&pex_msg) {
                                        let _ = framed.send(PeerMessage::Extended {
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
                msg_res = framed.next() => {
                    match msg_res {
                        Some(Ok(msg)) => {
                            self.handle_peer_message(
                                msg,
                                &mut framed,
                                &task,
                                meta_id,
                                sub_id,
                                &storage_tx,
                                &subtask_tx,
                                &mut peer_choking,
                            ).await?;
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
