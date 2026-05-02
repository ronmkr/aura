//! bt_worker: BitTorrent protocol implementation.

use crate::{Result, TaskId, Error};
use crate::worker::{ProtocolWorker, Segment, PieceData, ProgressSender};
use async_trait::async_trait;
use bytes::{Bytes, Buf, BufMut, BytesMut};
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::codec::{Decoder, Encoder, Framed};
use futures_util::{SinkExt, StreamExt};
use tracing::{info, debug, error};
use sha1::{Sha1, Digest};

pub const HANDSHAKE_LEN: usize = 68;
pub const PSTR: &[u8] = b"BitTorrent protocol";
pub const BLOCK_SIZE: u32 = 16384; // 16KB standard block size

pub type PeerId = [u8; 20];

/// Represents a BitTorrent handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Self { info_hash, peer_id }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HANDSHAKE_LEN);
        buf.push(PSTR.len() as u8);
        buf.extend_from_slice(PSTR);
        buf.extend_from_slice(&[0; 8]); // Reserved bytes
        buf.extend_from_slice(&self.info_hash);
        buf.extend_from_slice(&self.peer_id);
        buf
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < HANDSHAKE_LEN {
            return Err(Error::Protocol("Handshake too short".to_string()));
        }
        let pstr_len = data[0] as usize;
        if pstr_len != PSTR.len() || &data[1..1+pstr_len] != PSTR {
            return Err(Error::Protocol("Invalid protocol string".to_string()));
        }
        let mut info_hash = [0; 20];
        info_hash.copy_from_slice(&data[28..48]);
        let mut peer_id = [0; 20];
        peer_id.copy_from_slice(&data[48..68]);
        Ok(Self { info_hash, peer_id })
    }
}

/// BitTorrent peer messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerMessage {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request { index: u32, begin: u32, length: u32 },
    Piece { index: u32, begin: u32, block: Bytes },
    Cancel { index: u32, begin: u32, length: u32 },
}

impl PeerMessage {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            PeerMessage::KeepAlive => {
                buf.put_u32(0);
            }
            PeerMessage::Choke => {
                buf.put_u32(1);
                buf.put_u8(0);
            }
            PeerMessage::Unchoke => {
                buf.put_u32(1);
                buf.put_u8(1);
            }
            PeerMessage::Interested => {
                buf.put_u32(1);
                buf.put_u8(2);
            }
            PeerMessage::NotInterested => {
                buf.put_u32(1);
                buf.put_u8(3);
            }
            PeerMessage::Have(idx) => {
                buf.put_u32(5);
                buf.put_u8(4);
                buf.put_u32(*idx);
            }
            PeerMessage::Bitfield(bits) => {
                buf.put_u32(1 + bits.len() as u32);
                buf.put_u8(5);
                buf.extend_from_slice(bits);
            }
            PeerMessage::Request { index, begin, length } => {
                buf.put_u32(13);
                buf.put_u8(6);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
            }
            PeerMessage::Piece { index, begin, block } => {
                buf.put_u32(9 + block.len() as u32);
                buf.put_u8(7);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.extend_from_slice(block);
            }
            PeerMessage::Cancel { index, begin, length } => {
                buf.put_u32(13);
                buf.put_u8(8);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
            }
        }
        buf
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let id = data[0];
        let mut data = &data[1..];
        match id {
            0 => Ok(PeerMessage::Choke),
            1 => Ok(PeerMessage::Unchoke),
            2 => Ok(PeerMessage::Interested),
            3 => Ok(PeerMessage::NotInterested),
            4 => Ok(PeerMessage::Have(data.get_u32())),
            5 => Ok(PeerMessage::Bitfield(data.to_vec())),
            6 => Ok(PeerMessage::Request {
                index: data.get_u32(),
                begin: data.get_u32(),
                length: data.get_u32(),
            }),
            7 => {
                let index = data.get_u32();
                let begin = data.get_u32();
                Ok(PeerMessage::Piece {
                    index,
                    begin,
                    block: Bytes::copy_from_slice(data),
                })
            }
            8 => Ok(PeerMessage::Cancel {
                index: data.get_u32(),
                begin: data.get_u32(),
                length: data.get_u32(),
            }),
            _ => Err(Error::Protocol(format!("Unknown message ID: {}", id))),
        }
    }
}

pub struct PeerCodec;

impl Decoder for PeerCodec {
    type Item = PeerMessage;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        if src.len() < 4 {
            return Ok(None);
        }

        let length = u32::from_be_bytes([src[0], src[1], src[2], src[3]]) as usize;

        if length == 0 {
            src.advance(4);
            return Ok(Some(PeerMessage::KeepAlive));
        }

        if src.len() < 4 + length {
            src.reserve(4 + length - src.len());
            return Ok(None);
        }

        src.advance(4);
        let data = src.split_to(length);
        let msg = PeerMessage::deserialize(&data)?;
        Ok(Some(msg))
    }
}

impl Encoder<PeerMessage> for PeerCodec {
    type Error = Error;

    fn encode(&mut self, item: PeerMessage, dst: &mut BytesMut) -> Result<()> {
        let serialized = item.serialize();
        dst.extend_from_slice(&serialized);
        Ok(())
    }
}

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
        }
    }

    async fn connect_and_handshake(&self) -> Result<(TcpStream, [u8; 20])> {
        debug!(addr = %self.peer_addr, "Connecting to peer...");
        let remote_addr: std::net::SocketAddr = self.peer_addr.parse()
            .map_err(|e| Error::Protocol(format!("Invalid peer address {}: {}", self.peer_addr, e)))?;
        
        let mut stream = crate::net_util::connect_tcp_bound(remote_addr, None, self.local_addr).await?;

        debug!(addr = %self.peer_addr, "Sending handshake...");
        let handshake = Handshake::new(self.info_hash, self.my_id);
        stream.write_all(&handshake.serialize()).await?;

        let mut resp_data = [0u8; HANDSHAKE_LEN];
        stream.read_exact(&mut resp_data).await?;

        let resp_handshake = Handshake::deserialize(&resp_data)?;
        if resp_handshake.info_hash != self.info_hash {
            return Err(Error::Protocol("Info hash mismatch".to_string()));
        }

        info!(addr = %self.peer_addr, "Handshake successful");
        Ok((stream, resp_handshake.peer_id))
    }
}

#[async_trait]
impl ProtocolWorker for BtWorker {
    async fn fetch_segment(&self, _task_id: TaskId, segment: Segment, progress: Option<ProgressSender>) -> Result<PieceData> {
        let (stream, _peer_id) = self.connect_and_handshake().await?;
        let mut framed = Framed::new(stream, PeerCodec);
        let mut peer_choking = true;
        let mut _am_interested = false;

        let empty_bitfield = vec![0u8; 1];
        framed.send(PeerMessage::Bitfield(empty_bitfield)).await?;

        debug!(addr = %self.peer_addr, "Sending interested...");
        framed.send(PeerMessage::Interested).await?;
        _am_interested = true;

        let mut piece_buffer = BytesMut::with_capacity(segment.length as usize);
        piece_buffer.resize(segment.length as usize, 0);
        let mut bytes_received = 0;

        let piece_index = (segment.offset / segment.length) as u32;

        while bytes_received < segment.length {
            tokio::select! {
                msg_res = framed.next() => {
                    match msg_res {
                        Some(Ok(msg)) => {
                            debug!(addr = %self.peer_addr, ?msg, "Received message");
                            match msg {
                                PeerMessage::Choke => peer_choking = true,
                                PeerMessage::Unchoke => {
                                    peer_choking = false;
                                    info!(addr = %self.peer_addr, "Peer unchoked us, starting requests");
                                    self.request_next_block_internal(&mut framed, piece_index, bytes_received as u32, segment.length as u32).await?;
                                }
                                PeerMessage::Piece { index, begin, block } => {
                                    if index == piece_index {
                                        let len = block.len();
                                        piece_buffer[begin as usize..begin as usize + len].copy_from_slice(&block);
                                        bytes_received += len as u64;

                                        if let Some(ref p_tx) = progress {
                                            let _ = p_tx.send(len as u64);
                                        }

                                        if bytes_received < segment.length && !peer_choking {
                                            self.request_next_block_internal(&mut framed, piece_index, bytes_received as u32, segment.length as u32).await?;
                                        }
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

        if bytes_received == segment.length {
            info!(addr = %self.peer_addr, %piece_index, "Piece download complete");
            Ok(PieceData {
                segment,
                data: piece_buffer.freeze(),
            })
        } else {
            Err(Error::Protocol("Connection closed before piece complete".to_string()))
        }
    }

    fn available_capacity(&self) -> usize {
        1
    }
}

use std::sync::Arc;
use tokio::sync::mpsc;
use crate::bt_task::BtTask;
use crate::bitfield::Bitfield;
use crate::storage::StorageRequest;
use crate::orchestrator::SubTaskEvent;

impl BtWorker {
    pub async fn run_loop(
        mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        task: Arc<BtTask>,
        storage_tx: mpsc::Sender<StorageRequest>,
        subtask_tx: mpsc::UnboundedSender<SubTaskEvent>,
        token: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let (stream, peer_id) = self.connect_and_handshake().await?;
        self.peer_id = peer_id;
        self.run_loop_with_stream(stream, meta_id, sub_id, task, storage_tx, subtask_tx, token).await
    }

    pub async fn run_loop_with_stream(
        mut self,
        stream: TcpStream,
        meta_id: TaskId,
        _sub_id: TaskId,
        task: Arc<BtTask>,
        storage_tx: mpsc::Sender<StorageRequest>,
        subtask_tx: mpsc::UnboundedSender<SubTaskEvent>,
        token: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let mut framed = Framed::new(stream, PeerCodec);

        let mut peer_choking = true;
        let mut _am_interested = false;
        let am_choking = false;

        let my_bf = task.state.bitfield.lock().await;
        let my_bf_bytes = my_bf.as_bytes();
        framed.send(PeerMessage::Bitfield(my_bf_bytes)).await?;
        drop(my_bf);

        // Always unchoke for now
        framed.send(PeerMessage::Unchoke).await?;

        framed.send(PeerMessage::Interested).await?;
        _am_interested = true;

        let peer_addr = self.peer_addr.clone();
        let piece_length = task.state.torrent.info.piece_length;
        let total_length = task.state.torrent.total_length();

        info!(addr = %peer_addr, "Entering main peer message loop");

        let peer_addr_cleanup = peer_addr.clone();
        let task_cleanup = task.clone();
        let res = async move {
            let res: Result<()> = async {
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
                                            self.trigger_request(&mut framed, &task, piece_length, total_length, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                        }
                                        PeerMessage::Bitfield(bits) => {
                                            let bf = Bitfield::from_bytes(&bits, task.state.torrent.pieces_count());
                                            task.update_peer_state(&peer_addr, crate::peer_registry::ConnectionState::Handshaked).await;
                                            debug!(addr = %peer_addr, count = bf.count_set(), "Received bitfield");
                                            let mut picker = task.state.picker.lock().await;
                                            picker.add_peer_bitfield(peer_addr.clone(), bf.clone());
                                            drop(picker);
                                            let _ = subtask_tx.send(SubTaskEvent::PeerBitfield(meta_id, self.peer_id, bf));
                                            
                                            // After receiving a bitfield, we might be able to pick work
                                            if !peer_choking {
                                                self.trigger_request(&mut framed, &task, piece_length, total_length, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                            }
                                        }
                                        PeerMessage::Have(idx) => {
                                            let mut bf = Bitfield::new(task.state.torrent.pieces_count());
                                            bf.set(idx as usize, true);
                                            debug!(addr = %peer_addr, %idx, "Received Have");
                                            let mut picker = task.state.picker.lock().await;
                                            picker.add_peer_bitfield(peer_addr.clone(), bf);
                                            drop(picker);
                                            let _ = subtask_tx.send(SubTaskEvent::PeerHave(meta_id, self.peer_id, idx));
                                            
                                            // After receiving a have, we might be able to pick work
                                            if !peer_choking {
                                                self.trigger_request(&mut framed, &task, piece_length, total_length, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                            }
                                        }
                                        PeerMessage::Piece { index, begin, block } => {
                                            if Some(index as usize) == self.current_piece {
                                                let len = block.len();
                                                self.piece_buffer[begin as usize..begin as usize + len].copy_from_slice(&block);
                                                
                                                self.bytes_received += len as u64;
                                                let _ = subtask_tx.send(SubTaskEvent::Downloaded(meta_id, len as u64));

                                                if !peer_choking {
                                                    self.trigger_request(&mut framed, &task, piece_length, total_length, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                                }
                                            }
                                        }
                                        PeerMessage::Request { index, begin, length } => {
                                            if !am_choking {
                                                let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                                                let offset = index as u64 * piece_length + begin as u64;
                                                let read_req = StorageRequest::Read {
                                                    task_id: meta_id,
                                                    segment: Segment { offset, length: length as u64 },
                                                    reply_tx,
                                                };
                                                if storage_tx.send(read_req).await.is_ok() {
                                                    if let Ok(Ok(data)) = reply_rx.await {
                                                        framed.send(PeerMessage::Piece {
                                                            index,
                                                            begin,
                                                            block: data,
                                                        }).await?;
                                                    }
                                                }
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
            }.await;

            // Cleanup
            task_cleanup.update_peer_state(&peer_addr_cleanup, crate::peer_registry::ConnectionState::Disconnected).await;
            let mut picker = task_cleanup.state.picker.lock().await;
            if let Some(piece_idx) = self.current_piece {
                if self.bytes_received < piece_length {
                     picker.mark_completed(piece_idx); // Release piece
                }
            }
            picker.remove_peer(&peer_addr_cleanup);
            res
        }.await;

        res
    }

    async fn trigger_request<S>(
        &mut self, 
        framed: &mut Framed<S, PeerCodec>, 
        task: &BtTask,
        piece_length: u64,
        total_length: u64,
        meta_id: TaskId,
        storage_tx: mpsc::Sender<StorageRequest>,
        subtask_tx: mpsc::UnboundedSender<SubTaskEvent>,
    ) -> Result<()> 
    where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin
    {
        const MAX_IN_FLIGHT: u64 = 10 * BLOCK_SIZE as u64;

        if let Some(piece_idx) = self.current_piece {
            let piece_total_len = if piece_idx == task.state.torrent.pieces_count() - 1 {
                total_length - (piece_idx as u64 * piece_length)
            } else {
                piece_length
            };

            if self.bytes_received >= piece_total_len {
                // Piece complete, verify hash
                let mut hasher = Sha1::new();
                hasher.update(&self.piece_buffer);
                let actual_hash: [u8; 20] = hasher.finalize().into();
                let expected_hash = task.state.torrent.piece_hash(piece_idx)?;

                if actual_hash == expected_hash {
                    info!(addr = %self.peer_addr, %piece_idx, "Piece hash verified");
                    
                    let offset = piece_idx as u64 * piece_length;
                    let _ = storage_tx.send(StorageRequest::Write {
                        task_id: meta_id,
                        segment: Segment { offset, length: piece_total_len },
                        data: self.piece_buffer.clone().freeze(),
                    }).await;

                    {
                        let mut my_bf = task.state.bitfield.lock().await;
                        my_bf.set(piece_idx, true);
                    }

                    let mut picker = task.state.picker.lock().await;
                    picker.mark_completed(piece_idx);
                } else {
                    error!(addr = %self.peer_addr, %piece_idx, "Piece hash mismatch! Discarding piece.");
                    let mut picker = task.state.picker.lock().await;
                    picker.mark_completed(piece_idx);
                }

                self.current_piece = None;
                self.bytes_received = 0;
                self.bytes_requested = 0;
                self.piece_buffer.clear();
                
                return Box::pin(self.trigger_request(framed, task, piece_length, total_length, meta_id, storage_tx, subtask_tx)).await;
            }

            // Pipelining: fill up to MAX_IN_FLIGHT
            while (self.bytes_requested - self.bytes_received) < MAX_IN_FLIGHT && self.bytes_requested < piece_total_len {
                let length = std::cmp::min(BLOCK_SIZE, (piece_total_len - self.bytes_requested) as u32);
                debug!(addr = %self.peer_addr, %piece_idx, begin = self.bytes_requested, %length, "Requesting next block (pipelined)");
                
                framed.send(PeerMessage::Request {
                    index: piece_idx as u32,
                    begin: self.bytes_requested as u32,
                    length,
                }).await?;

                self.bytes_requested += length as u64;
            }
        } else {
            let bf = task.state.bitfield.lock().await;
            let mut picker = task.state.picker.lock().await;
            if let Some(piece_idx) = picker.pick_next(&bf, &self.peer_addr) {
                debug!(addr = %self.peer_addr, piece_idx, "Picker picked a new piece");
                picker.mark_in_progress(piece_idx);
                self.current_piece = Some(piece_idx);
                self.bytes_received = 0;
                self.bytes_requested = 0;
                
                let piece_total_len = if piece_idx == task.state.torrent.pieces_count() - 1 {
                    total_length - (piece_idx as u64 * piece_length)
                } else {
                    piece_length
                };
                
                self.piece_buffer.clear();
                self.piece_buffer.resize(piece_total_len as usize, 0);
                
                drop(picker);
                drop(bf);
                
                // Initial fill of pipeline
                return Box::pin(self.trigger_request(framed, task, piece_length, total_length, meta_id, storage_tx, subtask_tx)).await;
            } else {
                debug!(addr = %self.peer_addr, "Picker could not find any piece to pick");
            }
        }
        Ok(())
    }

    async fn request_next_block_internal<S>(&self, framed: &mut Framed<S, PeerCodec>, index: u32, begin: u32, total_len: u32) -> Result<()> 
    where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin 
    {
        let length = std::cmp::min(BLOCK_SIZE, total_len - begin);
        debug!(addr = %self.peer_addr, %index, %begin, %length, "Requesting block");
        framed.send(PeerMessage::Request {
            index,
            begin,
            length,
        }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handshake_serialization() {
        let h1 = Handshake::new([1; 20], [2; 20]);
        let serialized = h1.serialize();
        let h2 = Handshake::deserialize(&serialized).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_message_serialization() {
        let msg = PeerMessage::Have(123);
        let serialized = msg.serialize();
        let msg2 = PeerMessage::deserialize(&serialized[4..]).unwrap();
        assert_eq!(msg, msg2);
    }

    #[test]
    fn test_piece_message_serialization() {
        let block = Bytes::from(vec![1, 2, 3, 4]);
        let msg = PeerMessage::Piece { index: 10, begin: 20, block: block.clone() };
        let serialized = msg.serialize();
        let msg2 = PeerMessage::deserialize(&serialized[4..]).unwrap();
        if let PeerMessage::Piece { index, begin, block: block2 } = msg2 {
            assert_eq!(index, 10);
            assert_eq!(begin, 20);
            assert_eq!(block, block2);
        } else {
            panic!("Wrong message type");
        }
    }
}
