use super::protocol::{ExtendedHandshake, MetadataMessage, PeerCodec, PeerMessage};
use super::BtWorker;
use crate::orchestrator::SubTaskEvent;
use crate::storage::StorageRequest;
use crate::worker::bittorrent::task::BtTask;
use crate::{Result, TaskId};
use bytes::BytesMut;
use futures_util::SinkExt;
use sha2::Digest;
use tokio_util::codec::Framed;
use tracing::{debug, error};

impl BtWorker {
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn handle_peer_message<S>(
        &mut self,
        msg: PeerMessage,
        framed: &mut Framed<S, PeerCodec>,
        task: &BtTask,
        meta_id: TaskId,
        sub_id: TaskId,
        storage_tx: &tokio::sync::mpsc::Sender<StorageRequest>,
        subtask_tx: &tokio::sync::mpsc::Sender<SubTaskEvent>,
        peer_choking: &mut bool,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let peer_addr = self.peer_addr.clone();

        match msg {
            PeerMessage::Choke => {
                *peer_choking = true;
            }
            PeerMessage::Unchoke => {
                *peer_choking = false;
                debug!(addr = %peer_addr, "Peer unchoked us");
                self.trigger_request(
                    framed,
                    task,
                    meta_id,
                    sub_id,
                    storage_tx.clone(),
                    subtask_tx.clone(),
                )
                .await?;
            }
            PeerMessage::Bitfield(bits) => {
                let bf = crate::bitfield::Bitfield::from_bytes(
                    &bits,
                    task.state
                        .torrent
                        .lock()
                        .await
                        .as_ref()
                        .map(|t| t.pieces_count())
                        .unwrap_or(0),
                );
                task.update_peer_state(
                    &peer_addr,
                    crate::peer_registry::ConnectionState::Handshaked,
                )
                .await;
                let mut picker = task.state.picker.lock().await;
                if let Some(ref mut p) = *picker {
                    p.add_peer_bitfield(peer_addr.clone(), bf.clone());
                }
                drop(picker);
                let _ = subtask_tx
                    .send(SubTaskEvent::PeerBitfield(meta_id, self.peer_id, bf))
                    .await;

                if !*peer_choking {
                    self.trigger_request(
                        framed,
                        task,
                        meta_id,
                        sub_id,
                        storage_tx.clone(),
                        subtask_tx.clone(),
                    )
                    .await?;
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
                let _ = subtask_tx
                    .send(SubTaskEvent::PeerHave(meta_id, self.peer_id, idx))
                    .await;

                if !*peer_choking {
                    self.trigger_request(
                        framed,
                        task,
                        meta_id,
                        sub_id,
                        storage_tx.clone(),
                        subtask_tx.clone(),
                    )
                    .await?;
                }
            }
            PeerMessage::Extended { id, payload } => {
                if id == 0 {
                    if let Ok(hs) = serde_bencode::from_bytes::<ExtendedHandshake>(&payload) {
                        if let Some(ref m) = hs.m {
                            self.ut_metadata_id = m.get("ut_metadata").copied();
                            self.ut_pex_id = m.get("ut_pex").copied();
                            if let (Some(size), Some(_)) = (hs.metadata_size, self.ut_metadata_id) {
                                self.metadata_buffer = Some(BytesMut::zeroed(size));
                                self.trigger_request(
                                    framed,
                                    task,
                                    meta_id,
                                    sub_id,
                                    storage_tx.clone(),
                                    subtask_tx.clone(),
                                )
                                .await?;
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
                                        let mut full_torrent_dict =
                                            std::collections::HashMap::new();
                                        full_torrent_dict.insert(
                                            b"info".to_vec(),
                                            serde_bencode::value::Value::Bytes(
                                                full_info_dict.to_vec(),
                                            ),
                                        );
                                        full_torrent_dict.insert(
                                            b"announce".to_vec(),
                                            serde_bencode::value::Value::Bytes(
                                                b"http://aura-internal/".to_vec(),
                                            ),
                                        );
                                        if let Ok(torrent_bytes) = serde_bencode::to_bytes(
                                            &serde_bencode::value::Value::Dict(full_torrent_dict),
                                        ) {
                                            if let Ok(torrent) =
                                                crate::torrent::Torrent::from_bytes(&torrent_bytes)
                                            {
                                                if let Ok(Some(hash)) = torrent.info_hash_v1() {
                                                    if self.info_hash.matches_handshake(&hash) {
                                                        let _ = subtask_tx
                                                            .send(SubTaskEvent::MetadataReceived(
                                                                meta_id,
                                                                sub_id,
                                                                Box::new(torrent),
                                                            ))
                                                            .await;
                                                        self.trigger_request(
                                                            framed,
                                                            task,
                                                            meta_id,
                                                            sub_id,
                                                            storage_tx.clone(),
                                                            subtask_tx.clone(),
                                                        )
                                                        .await?;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if Some(id) == self.ut_pex_id {
                    if !self.pex_enabled {
                        return Ok(());
                    }
                    if let Ok(pex_msg) = serde_bencode::from_bytes::<
                        crate::worker::bittorrent::protocol::PexMessage,
                    >(&payload)
                    {
                        let peers = pex_msg.decode_peers();
                        if !peers.is_empty() {
                            let _ = subtask_tx
                                .send(SubTaskEvent::PexPeersDiscovered(self.info_hash, peers))
                                .await;
                        }
                    }
                }
            }
            PeerMessage::Request {
                index,
                begin,
                length,
            } => {
                let has_piece = {
                    let bf_guard = task.state.bitfield.lock().await;
                    bf_guard
                        .as_ref()
                        .map(|bf| bf.get(index as usize))
                        .unwrap_or(false)
                };

                if has_piece {
                    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                    let _ = storage_tx
                        .send(StorageRequest::Read {
                            task_id: meta_id,
                            segment: crate::worker::Segment {
                                offset: {
                                    let torrent_guard = task.state.torrent.lock().await;
                                    let torrent = torrent_guard.as_ref().unwrap();
                                    let base_offset = torrent
                                        .piece_align_offset(index as usize)
                                        .unwrap_or(index as u64 * torrent.info.piece_length);
                                    base_offset + begin as u64
                                },
                                length: length as u64,
                            },
                            reply_tx,
                        })
                        .await;

                    if let Ok(Ok(data)) = reply_rx.await {
                        framed
                            .send(PeerMessage::Piece {
                                index,
                                begin,
                                block: data,
                            })
                            .await?;
                        let _ = subtask_tx
                            .send(SubTaskEvent::Uploaded(
                                meta_id,
                                sub_id,
                                length as u64,
                                peer_addr.clone(),
                            ))
                            .await;
                    }
                }
            }
            PeerMessage::Interested => {
                task.update_peer_interest(&peer_addr, true).await;
            }
            PeerMessage::NotInterested => {
                task.update_peer_interest(&peer_addr, false).await;
            }
            PeerMessage::Piece {
                index,
                begin,
                block,
            } if Some(index as usize) == self.current_piece => {
                let len = block.len();
                // Admission Control: Wait for bandwidth tokens before processing the piece block
                self.throttler.acquire_download(meta_id, len as u64).await;

                // Block-level Merkle verification for v2
                let mut corrupted = false;
                let torrent_guard = task.state.torrent.lock().await;
                if let Some(ref torrent) = *torrent_guard {
                    if torrent.info.meta_version == Some(2) {
                        let block_idx_in_piece = (begin / 16384) as usize;
                        if let Ok(expected_block_hash) = torrent.block_hash_v2(
                            index as usize,
                            block_idx_in_piece,
                            Some(&task.state.db),
                        ) {
                            let mut hasher = sha2::Sha256::new();
                            hasher.update(&block);
                            let actual_block_hash: [u8; 32] = hasher.finalize().into();

                            if actual_block_hash != expected_block_hash {
                                error!(addr = %peer_addr, %index, begin, "Block hash mismatch! Discarding block.");
                                corrupted = true;
                            }
                        }
                    }
                }
                drop(torrent_guard);

                if corrupted {
                    // Reset piece download state to force re-request
                    if let Some(ref mut guard) = self.active_guard {
                        guard.complete();
                    }
                    self.active_guard = None;
                    self.current_piece = None;
                    self.bytes_received = 0;
                    self.bytes_requested = 0;
                    self.piece_buffer.clear();
                    return Ok(());
                }

                self.piece_buffer[begin as usize..begin as usize + len].copy_from_slice(&block);
                self.bytes_received += len as u64;

                // Send Downloaded event so PeerRegistry can track rates
                let _ = subtask_tx
                    .send(SubTaskEvent::Downloaded(
                        meta_id,
                        sub_id,
                        len as u64,
                        peer_addr.clone(),
                    ))
                    .await;

                if !*peer_choking {
                    self.trigger_request(
                        framed,
                        task,
                        meta_id,
                        sub_id,
                        storage_tx.clone(),
                        subtask_tx.clone(),
                    )
                    .await?;
                }
            }
            PeerMessage::Hashes {
                pieces_root,
                index,
                base: _,
                length: _,
                proof_layers: _,
                hashes,
            } => {
                debug!(addr = %peer_addr, ?pieces_root, index, "Received Merkle hashes from peer");
                let _ = storage_tx
                    .send(StorageRequest::StoreMerkleLayer {
                        pieces_root,
                        index,
                        hashes,
                    })
                    .await;
            }
            PeerMessage::HashRequest {
                pieces_root,
                index: _,
                base: _,
                length: _,
                proof_layers: _,
            } => {
                // TODO: Implement serving hashes to peers (Seed Mode)
                debug!(addr = %peer_addr, ?pieces_root, "Peer requested Merkle hashes (unsupported)");
            }
            _ => {}
        }

        Ok(())
    }
}
