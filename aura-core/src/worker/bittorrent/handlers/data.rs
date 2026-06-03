use super::super::protocol::PeerMessage;
use super::super::BtWorker;
use super::PeerHandlerContext;
use crate::orchestrator::SubTaskEvent;
use crate::storage::StorageRequest;
use crate::Result;
use bytes::Bytes;
use futures_util::SinkExt;
use sha2::Digest;
use tracing::{debug, error, info};

impl BtWorker {
    pub(crate) async fn handle_data_messages<S>(
        &mut self,
        msg: PeerMessage,
        ctx: &mut PeerHandlerContext<'_, S>,
    ) -> Result<bool>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let peer_addr = self.peer_addr.clone();

        match msg {
            PeerMessage::Request {
                index,
                begin,
                length,
            } => {
                self.handle_incoming_request(index, begin, length, ctx)
                    .await?;
                Ok(true)
            }
            PeerMessage::Piece {
                index,
                begin,
                block,
            } if Some(index as usize) == self.current_piece => {
                self.handle_incoming_piece(index, begin, block, ctx).await?;
                Ok(true)
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
                let _ = ctx
                    .storage_tx
                    .send(StorageRequest::StoreMerkleLayer {
                        pieces_root,
                        index,
                        hashes,
                    })
                    .await;
                Ok(true)
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
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    async fn handle_incoming_request<S>(
        &self,
        index: u32,
        begin: u32,
        length: u32,
        ctx: &mut PeerHandlerContext<'_, S>,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let has_piece = {
            let bf_guard = ctx.task.state.bitfield.lock().await;
            bf_guard
                .as_ref()
                .map(|bf| bf.get(index as usize))
                .unwrap_or(false)
        };

        if has_piece {
            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
            let _ = ctx
                .storage_tx
                .send(StorageRequest::Read {
                    task_id: ctx.meta_id,
                    segment: crate::worker::Segment {
                        offset: {
                            let torrent_guard = ctx.task.state.torrent.lock().await;
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
                ctx.framed
                    .send(PeerMessage::Piece {
                        index,
                        begin,
                        block: data,
                    })
                    .await?;
                let _ = ctx
                    .subtask_tx
                    .send(SubTaskEvent::Uploaded(
                        ctx.meta_id,
                        ctx.sub_id,
                        length as u64,
                        self.peer_addr.clone(),
                    ))
                    .await;
            }
        }
        Ok(())
    }

    async fn handle_incoming_piece<S>(
        &mut self,
        index: u32,
        begin: u32,
        block: Bytes,
        ctx: &mut PeerHandlerContext<'_, S>,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let peer_addr = self.peer_addr.clone();
        let len = block.len();
        // Admission Control: Wait for bandwidth tokens before processing the piece block
        self.throttler
            .acquire_download(ctx.meta_id, len as u64)
            .await;

        // Block-level Merkle verification for v2
        if !self.verify_block_v2(ctx.task, index, begin, &block).await? {
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
        let _ = ctx
            .subtask_tx
            .send(SubTaskEvent::Downloaded(
                ctx.meta_id,
                ctx.sub_id,
                len as u64,
                peer_addr.clone(),
            ))
            .await;

        // Check if piece is complete
        let piece_finished = {
            let torrent_guard = ctx.task.state.torrent.lock().await;
            if let Some(ref torrent) = *torrent_guard {
                let total_len = torrent.piece_actual_length(index as usize).unwrap_or(0);
                self.bytes_received >= total_len
            } else {
                false
            }
        };

        if piece_finished {
            let piece_idx = index as usize;
            info!(addr = %peer_addr, %piece_idx, "Piece download complete; verifying hash");

            let hash_valid = {
                let torrent_guard = ctx.task.state.torrent.lock().await;
                if let Some(ref torrent) = *torrent_guard {
                    if torrent.info.meta_version == Some(2) {
                        let expected = torrent.piece_hash_v2(piece_idx, Some(&ctx.task.state.db));
                        let mut hasher = sha2::Sha256::new();
                        hasher.update(&self.piece_buffer);
                        let actual: [u8; 32] = hasher.finalize().into();
                        expected.map(|e| e == actual).unwrap_or(false)
                    } else {
                        let expected = torrent.piece_hash_v1(piece_idx);
                        let mut hasher = sha1::Sha1::new();
                        hasher.update(&self.piece_buffer);
                        let actual: [u8; 20] = hasher.finalize().into();
                        expected.map(|e| e == actual).unwrap_or(false)
                    }
                } else {
                    false
                }
            };

            if hash_valid {
                info!(addr = %peer_addr, %piece_idx, "Hash verification successful");
                // Send to storage
                let finished_data = self.piece_buffer.clone();
                let offset = {
                    let torrent_guard = ctx.task.state.torrent.lock().await;
                    let torrent = torrent_guard.as_ref().unwrap();
                    torrent
                        .piece_align_offset(piece_idx)
                        .unwrap_or(piece_idx as u64 * torrent.info.piece_length)
                };

                let finished_len = finished_data.len() as u64;

                let _ = ctx
                    .storage_tx
                    .send(StorageRequest::Write {
                        task_id: ctx.meta_id,
                        segment: crate::worker::Segment {
                            offset,
                            length: finished_len,
                        },
                        data: finished_data,
                        guard: self.memory_guard.take(),
                        generation: Some(self.current_generation),
                    })
                    .await;

                // Mark in local bitfield
                if let Some(ref mut bf) = *ctx.task.state.bitfield.lock().await {
                    bf.set(piece_idx, true);
                }

                // Notify orchestrator
                let _ = ctx
                    .subtask_tx
                    .send(SubTaskEvent::RangeFinished(
                        ctx.meta_id,
                        ctx.sub_id,
                        crate::task::Range {
                            start: offset,
                            end: offset + finished_len,
                        },
                    ))
                    .await;

                if let Some(ref mut guard) = self.active_guard {
                    guard.complete();
                }
            } else {
                error!(addr = %peer_addr, %piece_idx, "Hash verification FAILED; discarding piece");
            }

            // Reset worker piece state
            self.memory_guard = None;
            self.active_guard = None;
            self.current_piece = None;
            self.bytes_received = 0;
            self.bytes_requested = 0;
            self.piece_buffer.clear();
        }

        if !*ctx.peer_choking {
            self.trigger_request(ctx).await?;
        }
        Ok(())
    }
}
