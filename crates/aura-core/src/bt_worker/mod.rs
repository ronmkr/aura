use crate::bt_task::BtTask;
use crate::orchestrator::SubTaskEvent;
use crate::storage::StorageRequest;
use crate::{Error, Result, TaskId};
use bytes::BytesMut;
use futures_util::{SinkExt, StreamExt};
use sha1::{Digest, Sha1};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

pub mod protocol;
pub use protocol::{
    ExtendedHandshake, Handshake, MetadataMessage, PeerCodec, PeerId, PeerMessage, BLOCK_SIZE,
    HANDSHAKE_LEN,
};

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
}

impl BtWorker {
    pub fn new(peer_addr: String, info_hash: [u8; 20], peer_id: [u8; 20], my_id: [u8; 20]) -> Self {
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

    pub async fn run_loop(
        mut self,
        _meta_id: TaskId,
        sub_id: TaskId,
        task: Arc<BtTask>,
        storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
        subtask_tx: tokio::sync::mpsc::Sender<SubTaskEvent>,
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
        token: CancellationToken,
    ) -> Result<()> {
        // Default to no extension support if not specified (legacy path)
        self.run_loop_with_stream_and_ext(
            stream, meta_id, sub_id, task, storage_tx, subtask_tx, token, false,
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
        token: CancellationToken,
        ext_support: bool,
    ) -> Result<()> {
        let mut framed = Framed::new(stream, PeerCodec);

        // If extension supported, send extended handshake
        if ext_support {
            let mut m = std::collections::HashMap::new();
            m.insert("ut_metadata".to_string(), 1);
            let ext_hs = ExtendedHandshake {
                m: Some(m),
                metadata_size: None,
            };
            let payload = serde_bencode::to_bytes(&ext_hs).unwrap();
            framed
                .send(PeerMessage::Extended {
                    id: 0,
                    payload: payload.into(),
                })
                .await?;
        }

        // Initial state
        framed.send(PeerMessage::Interested).await?;

        let mut peer_choking = true;
        let peer_addr = self.peer_addr.clone();

        info!(addr = %peer_addr, "Entering main peer message loop");

        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                msg_res = framed.next() => {
                    match msg_res {
                        Some(Ok(msg)) => {
                            match msg {
                                PeerMessage::Choke => {
                                    peer_choking = true;
                                }
                                PeerMessage::Unchoke => {
                                    peer_choking = false;
                                    debug!(addr = %peer_addr, "Peer unchoked us");
                                    self.trigger_request(&mut framed, &task, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                }
                                PeerMessage::Bitfield(bits) => {
                                    let bf = crate::bitfield::Bitfield::from_bytes(&bits, task.state.torrent.lock().await.as_ref().map(|t| t.pieces_count()).unwrap_or(0));
                                    task.update_peer_state(&peer_addr, crate::peer_registry::ConnectionState::Handshaked).await;
                                    debug!(addr = %peer_addr, count = bf.count_set(), "Received bitfield");
                                    let mut picker = task.state.picker.lock().await;
                                    if let Some(ref mut p) = *picker {
                                        p.add_peer_bitfield(peer_addr.clone(), bf.clone());
                                    }
                                    drop(picker);
                                    let _ = subtask_tx.send(SubTaskEvent::PeerBitfield(meta_id, self.peer_id, bf)).await;

                                    if !peer_choking {
                                        self.trigger_request(&mut framed, &task, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                    }
                                }
                                PeerMessage::Have(idx) => {
                                    debug!(addr = %peer_addr, %idx, "Received Have");
                                    let mut picker = task.state.picker.lock().await;
                                    if let Some(ref mut p) = *picker {
                                        let mut bf = crate::bitfield::Bitfield::new(p.num_pieces);
                                        bf.set(idx as usize, true);
                                        p.add_peer_bitfield(peer_addr.clone(), bf);
                                    }
                                    drop(picker);
                                    let _ = subtask_tx.send(SubTaskEvent::PeerHave(meta_id, self.peer_id, idx)).await;

                                    if !peer_choking {
                                        self.trigger_request(&mut framed, &task, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                    }
                                }
                                PeerMessage::Extended { id, payload } => {
                                    if id == 0 {
                                        // Extended handshake
                                        if let Ok(hs) = serde_bencode::from_bytes::<ExtendedHandshake>(&payload) {
                                            if let Some(m) = hs.m {
                                                self.ut_metadata_id = m.get("ut_metadata").cloned();
                                                if let (Some(size), Some(_)) = (hs.metadata_size, self.ut_metadata_id) {
                                                    info!(%peer_addr, %size, "Metadata size discovered from peer");
                                                    self.metadata_buffer = Some(BytesMut::zeroed(size));
                                                    self.trigger_request(&mut framed, &task, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                                }
                                            }
                                        }
                                    } else if Some(id) == self.ut_metadata_id {
                                        // Metadata message: [bencoded dict][data]
                                        // Find the end of the bencoded dict
                                        if let Some(pos) = payload.windows(2).position(|w| w == b"ee") {
                                            let bencoded_len = pos + 2;
                                            let bencoded = &payload[..bencoded_len];
                                            let data = &payload[bencoded_len..];

                                            if let Ok(msg) = serde_bencode::from_bytes::<MetadataMessage>(bencoded) {
                                                if msg.msg_type == 1 { // Data
                                                    if let Some(ref mut buf) = self.metadata_buffer {
                                                        let start = msg.piece as usize * 16384;
                                                        if start + data.len() <= buf.len() {
                                                            buf[start..start + data.len()].copy_from_slice(data);

                                                            // For now, assume single piece for metadata (very common)
                                                            // Assemble full torrent
                                                            let full_info_dict = buf.clone().freeze();
                                                            let mut full_torrent_dict = std::collections::HashMap::new();
                                                            full_torrent_dict.insert(b"info".to_vec(), serde_bencode::value::Value::Bytes(full_info_dict.to_vec()));
                                                            full_torrent_dict.insert(b"announce".to_vec(), serde_bencode::value::Value::Bytes(b"http://aura-internal/".to_vec()));

                                                            let torrent_bytes = serde_bencode::to_bytes(&serde_bencode::value::Value::Dict(full_torrent_dict)).unwrap();
                                                            if let Ok(torrent) = crate::torrent::Torrent::from_bytes(&torrent_bytes) {
                                                                if torrent.info_hash().unwrap() == self.info_hash {
                                                                    info!(%peer_addr, "Metadata successfully received and verified");
                                                                    let _ = subtask_tx.send(SubTaskEvent::MetadataReceived(meta_id, sub_id, torrent)).await;
                                                                    // After metadata, we might transition to piece picking
                                                                    self.trigger_request(&mut framed, &task, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                                                } else {
                                                                    warn!(%peer_addr, "Received metadata hash mismatch!");
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
                                        self.trigger_request(&mut framed, &task, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
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

    #[allow(clippy::too_many_arguments)]
    async fn trigger_request<S>(
        &mut self,
        framed: &mut Framed<S, PeerCodec>,
        task: &BtTask,
        meta_id: TaskId,
        storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
        subtask_tx: tokio::sync::mpsc::Sender<SubTaskEvent>,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        // 1. Check if we need metadata
        if task.state.torrent.lock().await.is_none() {
            if let Some(metadata_id) = self.ut_metadata_id {
                debug!(addr = %self.peer_addr, "Requesting metadata piece 0");
                let msg = MetadataMessage {
                    msg_type: 0,
                    piece: 0,
                    total_size: None,
                };
                let payload = serde_bencode::to_bytes(&msg).unwrap();
                framed
                    .send(PeerMessage::Extended {
                        id: metadata_id,
                        payload: payload.into(),
                    })
                    .await?;
            }
            return Ok(());
        }

        let torrent_guard = task.state.torrent.lock().await;
        let torrent = torrent_guard.as_ref().unwrap();
        let piece_length = torrent.info.piece_length;
        let total_length = torrent.total_length();

        let max_in_flight = self.pipeline_size as u64 * BLOCK_SIZE as u64;

        if let Some(piece_idx) = self.current_piece {
            let piece_total_len = if piece_idx == torrent.pieces_count() - 1 {
                total_length - (piece_idx as u64 * piece_length)
            } else {
                piece_length
            };

            if self.bytes_received >= piece_total_len {
                // Piece complete, verify hash
                let mut hasher = Sha1::new();
                hasher.update(&self.piece_buffer);
                let actual_hash: [u8; 20] = hasher.finalize().into();

                let expected_hash = &torrent.info.pieces[piece_idx * 20..(piece_idx + 1) * 20];

                if actual_hash == expected_hash {
                    info!(addr = %self.peer_addr, %piece_idx, "Piece download complete and verified");
                    let _ = storage_tx
                        .send(StorageRequest::Write {
                            task_id: meta_id,
                            segment: crate::worker::Segment {
                                offset: piece_idx as u64 * piece_length,
                                length: piece_total_len,
                            },
                            data: self.piece_buffer.clone().freeze(),
                        })
                        .await;

                    let mut bf_guard = task.state.bitfield.lock().await;
                    if let Some(ref mut bf) = *bf_guard {
                        bf.set(piece_idx, true);
                    }
                    let mut picker_guard = task.state.picker.lock().await;
                    if let Some(ref mut picker) = *picker_guard {
                        picker.mark_completed(piece_idx);
                    }
                } else {
                    error!(addr = %self.peer_addr, %piece_idx, "Piece hash mismatch!");
                    let mut picker_guard = task.state.picker.lock().await;
                    if let Some(ref mut picker) = *picker_guard {
                        picker.release_piece(piece_idx);
                    }
                }

                self.current_piece = None;
                self.bytes_received = 0;
                self.bytes_requested = 0;
                self.piece_buffer.clear();

                drop(torrent_guard);
                return Box::pin(
                    self.trigger_request(framed, task, meta_id, storage_tx, subtask_tx),
                )
                .await;
            }

            // Pipelining: fill up to MAX_IN_FLIGHT
            while (self.bytes_requested - self.bytes_received) < max_in_flight
                && self.bytes_requested < piece_total_len
            {
                let length =
                    std::cmp::min(BLOCK_SIZE, (piece_total_len - self.bytes_requested) as u32);
                debug!(addr = %self.peer_addr, %piece_idx, begin = self.bytes_requested, %length, "Requesting next block (pipelined)");

                framed
                    .send(PeerMessage::Request {
                        index: piece_idx as u32,
                        begin: self.bytes_requested as u32,
                        length,
                    })
                    .await?;
                self.bytes_requested += length as u64;
            }
        } else {
            // Try to pick a piece
            let bf_guard = task.state.bitfield.lock().await;
            let picker_guard = task.state.picker.lock().await;

            if let (Some(bf), Some(picker)) = (bf_guard.as_ref(), picker_guard.as_ref()) {
                if let Some(piece_idx) = picker.pick_next(bf, &self.peer_addr) {
                    let piece_total_len = if piece_idx == torrent.pieces_count() - 1 {
                        total_length - (piece_idx as u64 * piece_length)
                    } else {
                        piece_length
                    };

                    info!(addr = %self.peer_addr, %piece_idx, "Starting piece download");
                    self.current_piece = Some(piece_idx);
                    self.bytes_received = 0;
                    self.bytes_requested = 0;
                    self.piece_buffer = BytesMut::zeroed(piece_total_len as usize);

                    drop(picker_guard);
                    drop(bf_guard);
                    drop(torrent_guard);
                    return Box::pin(
                        self.trigger_request(framed, task, meta_id, storage_tx, subtask_tx),
                    )
                    .await;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use tokio_util::codec::{Decoder, Encoder};

    #[test]
    fn test_handshake_serialization() {
        let info_hash = [1u8; 20];
        let peer_id = [2u8; 20];
        let handshake = Handshake::new(info_hash, peer_id);
        let serialized = handshake.serialize();
        let deserialized = Handshake::deserialize(&serialized).unwrap();
        assert_eq!(handshake.info_hash, deserialized.info_hash);
        assert_eq!(handshake.peer_id, deserialized.peer_id);
        assert!(deserialized.extension_protocol);
    }

    #[test]
    fn test_message_serialization() {
        let msg = PeerMessage::Have(123);
        let serialized = msg.serialize();
        let deserialized = PeerMessage::deserialize(&serialized[4..]).unwrap(); // Skip length prefix
        assert_eq!(msg, deserialized);
    }

    #[test]
    fn test_piece_message_serialization() {
        let block = Bytes::from(vec![1, 2, 3, 4]);
        let msg = PeerMessage::Piece {
            index: 1,
            begin: 0,
            block: block.clone(),
        };
        let mut buf = BytesMut::new();
        let mut codec = PeerCodec;
        codec.encode(msg.clone(), &mut buf).unwrap();
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(msg, decoded);
    }
}
