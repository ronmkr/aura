use crate::bt_task::BtTask;
use crate::orchestrator::{SubTaskEvent, WorkerCommand};
use crate::storage::StorageRequest;
use crate::{Error, Result, TaskId};
use bytes::BytesMut;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

pub mod logic;
pub mod protocol;

#[cfg(test)]
mod tests;

pub use protocol::{
    ExtendedHandshake, Handshake, MetadataMessage, PeerCodec, PeerId, PeerMessage, BLOCK_SIZE,
    HANDSHAKE_LEN,
};

use crate::buffer_pool::BufferPool;

pub struct BtWorker {
    pub peer_addr: String,
    pub info_hash: [u8; 20],
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
    pub pool: BufferPool,
}

impl BtWorker {
    pub fn new(
        peer_addr: String,
        info_hash: [u8; 20],
        peer_id: [u8; 20],
        my_id: [u8; 20],
        pool: BufferPool,
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
            pool,
        }
    }

    async fn connect_and_handshake(&self) -> Result<(TcpStream, [u8; 20], bool)> {
        debug!(addr = %self.peer_addr, "Connecting to peer...");
        let remote_addr: std::net::SocketAddr = self.peer_addr.parse().map_err(|e| {
            Error::Protocol(format!("Invalid peer address {}: {}", self.peer_addr, e))
        })?;

        let mut stream =
            crate::net_util::connect_tcp_bound(remote_addr, None, self.local_addr).await?;

        debug!(addr = %self.peer_addr, "Sending handshake...");
        let handshake = Handshake::new(self.info_hash, self.my_id);
        stream.write_all(&handshake.serialize()).await?;

        let mut buf = [0u8; HANDSHAKE_LEN];
        use tokio::io::AsyncReadExt;
        stream.read_exact(&mut buf).await?;
        let res_handshake = Handshake::deserialize(&buf)?;

        if res_handshake.info_hash != self.info_hash {
            return Err(Error::Protocol("Handshake info_hash mismatch".to_string()));
        }

        Ok((
            stream,
            res_handshake.peer_id,
            res_handshake.extension_protocol,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_loop(
        mut self,
        _meta_id: TaskId,
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
            _meta_id,
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
                                // Initialization of piece_buffer and fetching logic is handled in trigger_request
                                self.trigger_request(&mut framed, &task, meta_id, sub_id, storage_tx.clone(), subtask_tx.clone()).await?;
                            }
                        }
                    }
                }
                msg_res = framed.next() => {
                    match msg_res {
                        Some(Ok(msg)) => {
                            use crate::bt_worker::protocol::PeerMessage;
                            match msg {
                                PeerMessage::Choke => {
                                    peer_choking = true;
                                }
                                PeerMessage::Unchoke => {
                                    peer_choking = false;
                                    debug!(addr = %peer_addr, "Peer unchoked us");
                                    self.trigger_request(&mut framed, &task, meta_id, sub_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                }
                                PeerMessage::Bitfield(bits) => {
                                    let bf = crate::bitfield::Bitfield::from_bytes(&bits, task.state.torrent.lock().await.as_ref().map(|t| t.pieces_count()).unwrap_or(0));
                                    task.update_peer_state(&peer_addr, crate::peer_registry::ConnectionState::Handshaked).await;
                                    let mut picker = task.state.picker.lock().await;
                                    if let Some(ref mut p) = *picker {
                                        p.add_peer_bitfield(peer_addr.clone(), bf.clone());
                                    }
                                    drop(picker);
                                    let _ = subtask_tx.send(SubTaskEvent::PeerBitfield(meta_id, self.peer_id, bf)).await;

                                    if !peer_choking {
                                        self.trigger_request(&mut framed, &task, meta_id, sub_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                    }
                                }
                                PeerMessage::Have(idx) => {
                                    let mut picker = task.state.picker.lock().await;
                                    if let Some(ref mut p) = *picker {
                                        let mut bf = crate::bitfield::Bitfield::new(p.num_pieces);
                                        bf.set(idx as usize, true);
                                        p.add_peer_bitfield(peer_addr.clone(), bf);
                                    }
                                    drop(picker);
                                    let _ = subtask_tx.send(SubTaskEvent::PeerHave(meta_id, self.peer_id, idx)).await;

                                    if !peer_choking {
                                        self.trigger_request(&mut framed, &task, meta_id, sub_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                    }
                                }
                                PeerMessage::Extended { id, payload } => {
                                    if id == 0 {
                                        if let Ok(hs) = serde_bencode::from_bytes::<ExtendedHandshake>(&payload) {
                                            if let Some(m) = hs.m {
                                                self.ut_metadata_id = m.get("ut_metadata").cloned();
                                                if let (Some(size), Some(_)) = (hs.metadata_size, self.ut_metadata_id) {
                                                    self.metadata_buffer = Some(BytesMut::zeroed(size));
                                                    self.trigger_request(&mut framed, &task, meta_id, sub_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                                }
                                            }
                                        }
                                    } else if Some(id) == self.ut_metadata_id {
                                        if let Some(pos) = payload.windows(2).position(|w| w == b"ee") {
                                            let bencoded_len = pos + 2;
                                            let bencoded = &payload[..bencoded_len];
                                            let data = &payload[bencoded_len..];

                                            if let Ok(msg) = serde_bencode::from_bytes::<MetadataMessage>(bencoded) {
                                                if msg.msg_type == 1 {
                                                    if let Some(ref mut buf) = self.metadata_buffer {
                                                        let start = msg.piece as usize * 16384;
                                                        if start + data.len() <= buf.len() {
                                                            buf[start..start + data.len()].copy_from_slice(data);
                                                            let full_info_dict = buf.clone().freeze();
                                                            let mut full_torrent_dict = std::collections::HashMap::new();
                                                            full_torrent_dict.insert(b"info".to_vec(), serde_bencode::value::Value::Bytes(full_info_dict.to_vec()));
                                                            full_torrent_dict.insert(b"announce".to_vec(), serde_bencode::value::Value::Bytes(b"http://aura-internal/".to_vec()));
                                                            if let Ok(torrent_bytes) = serde_bencode::to_bytes(&serde_bencode::value::Value::Dict(full_torrent_dict)) {
                                                                if let Ok(torrent) = crate::torrent::Torrent::from_bytes(&torrent_bytes) {
                                                                    if let Ok(hash) = torrent.info_hash() {
                                                                        if hash == self.info_hash {
                                                                            let _ = subtask_tx.send(SubTaskEvent::MetadataReceived(meta_id, sub_id, torrent)).await;
                                                                            self.trigger_request(&mut framed, &task, meta_id, sub_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                PeerMessage::Piece { index, begin, block }
                                    if Some(index as usize) == self.current_piece =>
                                {
                                    let len = block.len();
                                    self.piece_buffer[begin as usize..begin as usize + len].copy_from_slice(&block);
                                    self.bytes_received += len as u64;
                                    let _ = subtask_tx.send(SubTaskEvent::Downloaded(meta_id, len as u64)).await;

                                    if !peer_choking {
                                        self.trigger_request(&mut framed, &task, meta_id, sub_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                    }
                                }
                                _ => {}
                            }
                        }
                        Some(Err(e)) => return Err(e),
                        None => break,
                    }
                }
            }
        }
        Ok(())
    }
}
