use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use tokio_util::codec::Framed;
use tokio_util::sync::CancellationToken;
use futures_util::{SinkExt, StreamExt};
use tracing::{info, debug, error};
use sha1::{Sha1, Digest};
use bytes::BytesMut;
use crate::{Result, TaskId, Error};
use crate::storage::StorageRequest;
use crate::orchestrator::SubTaskEvent;
use crate::bt_task::BtTask;

pub mod protocol;
pub use protocol::{PeerId, Handshake, PeerMessage, PeerCodec, HANDSHAKE_LEN, BLOCK_SIZE};

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

        let mut buf = [0u8; HANDSHAKE_LEN];
        use tokio::io::AsyncReadExt;
        stream.read_exact(&mut buf).await?;
        let res_handshake = Handshake::deserialize(&buf)?;

        if res_handshake.info_hash != self.info_hash {
            return Err(Error::Protocol("Handshake info_hash mismatch".to_string()));
        }

        Ok((stream, res_handshake.peer_id))
    }

    pub async fn run_loop(
        mut self,
        _meta_id: TaskId,
        sub_id: TaskId,
        task: Arc<BtTask>,
        storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
        subtask_tx: tokio::sync::mpsc::UnboundedSender<SubTaskEvent>,
        token: CancellationToken,
    ) -> Result<()> {
        let (stream, peer_id) = self.connect_and_handshake().await?;
        self.peer_id = peer_id;
        
        self.run_loop_with_stream(stream, _meta_id, sub_id, task, storage_tx, subtask_tx, token).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn run_loop_with_stream(
        mut self,
        stream: TcpStream,
        meta_id: TaskId,
        _sub_id: TaskId,
        task: Arc<BtTask>,
        storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
        subtask_tx: tokio::sync::mpsc::UnboundedSender<SubTaskEvent>,
        token: CancellationToken,
    ) -> Result<()> {
        let mut framed = Framed::new(stream, PeerCodec);
        
        // Initial state
        framed.send(PeerMessage::Interested).await?;
        
        let mut peer_choking = true;
        let peer_addr = self.peer_addr.clone();

        let piece_length = task.state.torrent.info.piece_length;
        let total_length = task.state.torrent.total_length();

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
                                    self.trigger_request(&mut framed, &task, piece_length, total_length, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                }
                                PeerMessage::Bitfield(bits) => {
                                    let bf = crate::bitfield::Bitfield::from_bytes(&bits, task.state.torrent.pieces_count());
                                    task.update_peer_state(&peer_addr, crate::peer_registry::ConnectionState::Handshaked).await;
                                    debug!(addr = %peer_addr, count = bf.count_set(), "Received bitfield");
                                    let mut picker = task.state.picker.lock().await;
                                    picker.add_peer_bitfield(peer_addr.clone(), bf.clone());
                                    drop(picker);
                                    let _ = subtask_tx.send(SubTaskEvent::PeerBitfield(meta_id, self.peer_id, bf));
                                    
                                    if !peer_choking {
                                        self.trigger_request(&mut framed, &task, piece_length, total_length, meta_id, storage_tx.clone(), subtask_tx.clone()).await?;
                                    }
                                }
                                PeerMessage::Have(idx) => {
                                    let mut bf = crate::bitfield::Bitfield::new(task.state.torrent.pieces_count());
                                    bf.set(idx as usize, true);
                                    debug!(addr = %peer_addr, %idx, "Received Have");
                                    let mut picker = task.state.picker.lock().await;
                                    picker.add_peer_bitfield(peer_addr.clone(), bf);
                                    drop(picker);
                                    let _ = subtask_tx.send(SubTaskEvent::PeerHave(meta_id, self.peer_id, idx));
                                    
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
                                    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                                    let offset = index as u64 * piece_length + begin as u64;
                                    let read_req = StorageRequest::Read {
                                        task_id: meta_id,
                                        segment: crate::worker::Segment { offset, length: length as u64 },
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
        piece_length: u64,
        total_length: u64,
        meta_id: TaskId,
        storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
        subtask_tx: tokio::sync::mpsc::UnboundedSender<SubTaskEvent>,
    ) -> Result<()> 
    where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin
    {
        let max_in_flight = self.pipeline_size as u64 * BLOCK_SIZE as u64;

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
                
                let expected_hash = &task.state.torrent.info.pieces[piece_idx * 20..(piece_idx + 1) * 20];
                
                if actual_hash == expected_hash {
                    info!(addr = %self.peer_addr, %piece_idx, "Piece download complete and verified");
                    let _ = storage_tx.send(StorageRequest::Write {
                        task_id: meta_id,
                        segment: crate::worker::Segment {
                            offset: piece_idx as u64 * piece_length,
                            length: piece_total_len,
                        },
                        data: self.piece_buffer.clone().freeze(),
                    }).await;

                    let mut bf = task.state.bitfield.lock().await;
                    bf.set(piece_idx, true);
                    let mut picker = task.state.picker.lock().await;
                    picker.mark_completed(piece_idx);
                } else {
                    error!(addr = %self.peer_addr, %piece_idx, "Piece hash mismatch!");
                    let mut picker = task.state.picker.lock().await;
                    picker.release_piece(piece_idx);
                }

                self.current_piece = None;
                self.bytes_received = 0;
                self.bytes_requested = 0;
                self.piece_buffer.clear();
                
                return Box::pin(self.trigger_request(framed, task, piece_length, total_length, meta_id, storage_tx, subtask_tx)).await;
            }

            // Pipelining: fill up to MAX_IN_FLIGHT
            while (self.bytes_requested - self.bytes_received) < max_in_flight && self.bytes_requested < piece_total_len {
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
            // Pick a new piece
            let my_bf = task.state.bitfield.lock().await;
            let picker = task.state.picker.lock().await;
            if let Some(piece_idx) = picker.pick_next(&my_bf, &self.peer_addr) {
                let piece_total_len = if piece_idx == task.state.torrent.pieces_count() - 1 {
                    total_length - (piece_idx as u64 * piece_length)
                } else {
                    piece_length
                };

                info!(addr = %self.peer_addr, %piece_idx, "Starting piece download");
                self.current_piece = Some(piece_idx);
                self.bytes_received = 0;
                self.bytes_requested = 0;
                self.piece_buffer = BytesMut::zeroed(piece_total_len as usize);
                
                drop(picker);
                drop(my_bf);
                return Box::pin(self.trigger_request(framed, task, piece_length, total_length, meta_id, storage_tx, subtask_tx)).await;
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
        assert_eq!(handshake, deserialized);
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
        let msg = PeerMessage::Piece { index: 1, begin: 0, block: block.clone() };
        let mut buf = BytesMut::new();
        let mut codec = PeerCodec;
        codec.encode(msg.clone(), &mut buf).unwrap();
        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(msg, decoded);
    }
}
