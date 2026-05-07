use super::protocol::{MetadataMessage, PeerCodec, PeerMessage, BLOCK_SIZE};
use crate::bt_task::BtTask;
use crate::orchestrator::SubTaskEvent;
use crate::storage::StorageRequest;
use crate::{Error, Result, TaskId};
use futures_util::SinkExt;
use sha1::{Digest, Sha1};
use sha2::Sha256;
use tokio_util::codec::Framed;
use tracing::{debug, error, info};

impl super::BtWorker {
    #[allow(clippy::too_many_arguments)]
    pub async fn trigger_request<S>(
        &mut self,
        framed: &mut Framed<S, PeerCodec>,
        task: &BtTask,
        meta_id: TaskId,
        sub_id: TaskId,
        storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
        subtask_tx: tokio::sync::mpsc::Sender<SubTaskEvent>,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let torrent_guard = task.state.torrent.lock().await;
        let torrent = match torrent_guard.as_ref() {
            Some(t) => t,
            None => {
                if let Some(metadata_id) = self.ut_metadata_id {
                    debug!(addr = %self.peer_addr, "Requesting metadata piece 0");
                    let msg = MetadataMessage {
                        msg_type: 0,
                        piece: 0,
                        total_size: None,
                    };
                    let payload = serde_bencode::to_bytes(&msg).map_err(|e| {
                        Error::Protocol(format!("Failed to encode metadata request: {}", e))
                    })?;
                    framed
                        .send(PeerMessage::Extended {
                            id: metadata_id,
                            payload: payload.into(),
                        })
                        .await?;
                }
                return Ok(());
            }
        };

        if let Some(piece_idx) = self.current_piece {
            let finished = {
                let bf_guard = task.state.bitfield.lock().await;
                bf_guard
                    .as_ref()
                    .map(|bf| bf.get(piece_idx))
                    .unwrap_or(false)
            };
            if finished {
                debug!(addr = %self.peer_addr, %piece_idx, "Piece finished by another worker, dropping");
                self.current_piece = None;
                self.bytes_received = 0;
                self.bytes_requested = 0;
                self.piece_buffer.clear();
                drop(torrent_guard);
                return Box::pin(
                    self.trigger_request(framed, task, meta_id, sub_id, storage_tx, subtask_tx),
                )
                .await;
            }
        }

        let piece_length = torrent.info.piece_length;
        let total_length = torrent.total_length();

        let max_in_flight = self.pipeline_size as u64 * BLOCK_SIZE as u64;

        if let Some(piece_idx) = self.current_piece {
            let piece_total_len = if piece_idx == torrent.pieces_count() - 1 {
                total_length - (piece_idx as u64 * piece_length)
            } else {
                piece_length
            };

            // Ensure buffer is initialized if it was set via RequestPiece (Endgame)
            if self.piece_buffer.is_empty() {
                self.piece_buffer = self.pool.acquire();
                self.piece_buffer.resize(piece_total_len as usize, 0);
            }

            if self.bytes_received >= piece_total_len {
                // Piece complete, verify hash
                let is_verified = if torrent.info_hash_v2().unwrap_or(None).is_some() {
                    // v2 or Hybrid: use SHA-256
                    let mut hasher = Sha256::new();
                    hasher.update(&self.piece_buffer);
                    let actual_hash: [u8; 32] = hasher.finalize().into();

                    if let Ok(expected_hash) =
                        torrent.piece_hash_v2(piece_idx, Some(&task.state.db))
                    {
                        actual_hash == expected_hash
                    } else if let Ok(expected_hash1) = torrent.piece_hash_v1(piece_idx) {
                        // Fallback to v1 hash if v2 piece hash lookup fails (e.g., hybrid padding)
                        let mut hasher1 = Sha1::new();
                        hasher1.update(&self.piece_buffer);
                        let actual_hash1: [u8; 20] = hasher1.finalize().into();
                        actual_hash1 == expected_hash1
                    } else {
                        false
                    }
                } else {
                    // v1-only
                    if let Ok(expected_hash) = torrent.piece_hash_v1(piece_idx) {
                        let mut hasher = Sha1::new();
                        hasher.update(&self.piece_buffer);
                        let actual_hash: [u8; 20] = hasher.finalize().into();
                        actual_hash == expected_hash
                    } else {
                        false
                    }
                };

                if is_verified {
                    info!(addr = %self.peer_addr, %piece_idx, "Piece download complete and verified");

                    let finished_data =
                        std::mem::replace(&mut self.piece_buffer, self.pool.acquire());

                    let _ = storage_tx
                        .send(StorageRequest::Write {
                            task_id: meta_id,
                            segment: crate::worker::Segment {
                                offset: piece_idx as u64 * piece_length,
                                length: piece_total_len,
                            },
                            data: finished_data,
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
                    drop(picker_guard);

                    let _ = subtask_tx
                        .send(SubTaskEvent::PieceVerified(meta_id, sub_id, piece_idx))
                        .await;
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
                    self.trigger_request(framed, task, meta_id, sub_id, storage_tx, subtask_tx),
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
                    self.piece_buffer = self.pool.acquire();
                    self.piece_buffer.resize(piece_total_len as usize, 0);

                    drop(picker_guard);
                    drop(bf_guard);
                    drop(torrent_guard);
                    return Box::pin(
                        self.trigger_request(framed, task, meta_id, sub_id, storage_tx, subtask_tx),
                    )
                    .await;
                }
            }
        }

        Ok(())
    }
}
