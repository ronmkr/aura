use super::handlers::PeerHandlerContext;
use super::protocol::{MetadataMessage, PeerMessage, BLOCK_SIZE};
use crate::{Error, Result};
use bytes::BytesMut;
use futures_util::SinkExt;
use tracing::{debug, info, warn};

impl super::BtWorker {
    pub(crate) async fn trigger_request<S>(
        &mut self,
        ctx: &mut PeerHandlerContext<'_, S>,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let torrent_guard: tokio::sync::MutexGuard<Option<crate::torrent::Torrent>> =
            ctx.task.state.torrent.lock().await;
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
                    ctx.framed
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
                let bf_guard: tokio::sync::MutexGuard<Option<crate::bitfield::Bitfield>> =
                    ctx.task.state.bitfield.lock().await;
                bf_guard
                    .as_ref()
                    .map(|bf| bf.get(piece_idx))
                    .unwrap_or(false)
            };
            if finished {
                debug!(addr = %self.peer_addr, %piece_idx, "Piece finished by another worker, dropping");
                if let Some(ref mut guard) = self.active_guard {
                    guard.complete();
                }
                self.active_guard = None;
                self.current_piece = None;
                self.bytes_received = 0;
                self.bytes_requested = 0;
                self.piece_buffer.clear();
                drop(torrent_guard);
                return Box::pin(self.trigger_request(ctx)).await;
            }
        }

        let piece_length = torrent.info.piece_length;
        let total_length = torrent.total_length();

        let max_in_flight = self.pipeline_size as u64 * BLOCK_SIZE as u64;

        if let Some(piece_idx) = self.current_piece {
            let piece_total_len = torrent
                .piece_actual_length(piece_idx)
                .unwrap_or(piece_length);

            // Ensure buffer is initialized if it was set via RequestPiece (Endgame)
            if self.piece_buffer.is_empty() {
                self.piece_buffer = BytesMut::with_capacity(
                    crate::worker::bittorrent::protocol::BLOCK_SIZE as usize,
                );
                self.piece_buffer.resize(piece_total_len as usize, 0);
            }

            // Piece completion and hash verification are handled by
            // handle_incoming_piece in handlers/data.rs when the final
            // block arrives. This method only pipelines block requests.

            // Pipelining: fill up to MAX_IN_FLIGHT
            while (self.bytes_requested - self.bytes_received) < max_in_flight
                && self.bytes_requested < piece_total_len
            {
                let length =
                    std::cmp::min(BLOCK_SIZE, (piece_total_len - self.bytes_requested) as u32);
                info!(addr = %self.peer_addr, %piece_idx, begin = self.bytes_requested, %length, "Requesting block (pipelined)");

                ctx.framed
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
            // Lock order: Torrent -> Bitfield -> Picker (Consistent with Scrubber to avoid deadlock)
            // We already hold the torrent lock from the start of the method
            let bf_guard: tokio::sync::MutexGuard<Option<crate::bitfield::Bitfield>> =
                ctx.task.state.bitfield.lock().await;
            let mut picker_guard: tokio::sync::MutexGuard<
                Option<crate::piece_picker::PiecePicker>,
            > = ctx.task.state.picker.lock().await;

            if let (Some(bf), Some(picker)) = (bf_guard.as_ref(), picker_guard.as_mut()) {
                let sequential = ctx
                    .task
                    .state
                    .sequential
                    .load(std::sync::atomic::Ordering::Relaxed);

                if let Some(piece_idx) = picker.pick_next(bf, &self.peer_addr, sequential) {
                    let piece_total_len = if piece_idx == torrent.pieces_count() - 1 {
                        total_length - (piece_idx as u64 * piece_length)
                    } else {
                        piece_length
                    };

                    if !ctx
                        .task
                        .state
                        .resource_governor
                        .request_allocation(&ctx.task.state.tenant_id, piece_total_len as usize)
                    {
                        picker.release_piece(piece_idx);
                        drop(picker_guard);
                        drop(bf_guard);
                        drop(torrent_guard);
                        info!(addr = %self.peer_addr, %piece_idx, "Memory allocation request failed. Releasing piece and backing off.");
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        return Ok(());
                    }

                    self.memory_guard =
                        Some(crate::orchestrator::resource_governor::MemoryGuard::new(
                            ctx.task.state.resource_governor.clone(),
                            ctx.task.state.tenant_id.clone(),
                            piece_total_len as usize,
                        ));

                    {
                        let mut gens = ctx.task.state.generations.lock().await;
                        let entry = gens.entry(piece_idx).or_insert(0);
                        *entry += 1;
                        self.current_generation = *entry;
                    }

                    // Check if we need to request block hashes for this file
                    self.check_and_request_hashes_internal(
                        ctx.framed, ctx.task, torrent, piece_idx,
                    )
                    .await?;

                    info!(addr = %self.peer_addr, %piece_idx, "Starting piece download");
                    self.current_piece = Some(piece_idx);
                    self.bytes_received = 0;
                    self.bytes_requested = 0;
                    self.piece_buffer = BytesMut::with_capacity(piece_total_len as usize);
                    self.piece_buffer.resize(piece_total_len as usize, 0);

                    let state_clone = ctx.task.state.clone();
                    let guard = crate::piece_picker::PieceGuard::new(piece_idx, move |idx| {
                        tokio::spawn(async move {
                            let mut pg: tokio::sync::MutexGuard<
                                Option<crate::piece_picker::PiecePicker>,
                            > = state_clone.picker.lock().await;
                            if let Some(picker) = pg.as_mut() {
                                picker.release_piece(idx);
                            }
                        });
                    });
                    self.active_guard = Some(guard);

                    drop(picker_guard);
                    drop(bf_guard);
                    drop(torrent_guard);
                    return Box::pin(self.trigger_request(ctx)).await;
                } else {
                    info!(addr = %self.peer_addr, "No piece picked by picker (possibly peer has no pieces we need)");
                }
            } else {
                warn!(addr = %self.peer_addr, "Bitfield or Picker missing during trigger_request");
            }
        }

        Ok(())
    }
}
