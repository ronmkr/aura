use super::types::{BtWorker, BtWorkerArgs};
use crate::orchestrator::WorkerCommand;
use crate::worker::bittorrent::handlers::PeerHandlerContext;
use crate::worker::bittorrent::protocol::mse::MseStream;
use crate::worker::bittorrent::protocol::{ExtendedHandshake, PeerCodec, PeerMessage};
use crate::{Error, Result};
use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use tracing::{debug, info};

impl BtWorker {
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
        stream: MseStream<TcpStream>,
        args: BtWorkerArgs,
        ext_support: bool,
    ) -> Result<()> {
        self.run_loop_with_stream_and_ext(stream, args, ext_support)
            .await
    }

    async fn run_loop_with_stream_and_ext(
        &mut self,
        stream: MseStream<TcpStream>,
        args: BtWorkerArgs,
        ext_support: bool,
    ) -> Result<()> {
        let BtWorkerArgs {
            meta_id,
            sub_id,
            task,
            storage_client,
            subtask_tx,
            mut command_rx,
            token,
        } = args;

        let mut framed = Framed::new(stream, PeerCodec);

        // Send Bitfield
        if let Some(bf) = task.state.bitfield.lock().await.as_ref() {
            framed.send(PeerMessage::Bitfield(bf.as_bytes())).await?;
        }

        if ext_support {
            let is_private = if let Some(ref torrent) = *task.state.torrent.lock().await {
                torrent.is_private()
            } else {
                false
            };
            let mut m = std::collections::HashMap::new();
            m.insert("ut_metadata".to_string(), 1);
            if self.pex_enabled && !is_private {
                m.insert("ut_pex".to_string(), 2);
            }
            let ext_hs = ExtendedHandshake {
                m: Some(m),
                metadata_size: None, // Will be filled if needed
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
                cmd_res = command_rx.recv() => {
                    let cmd = match cmd_res {
                        Ok(c) => c,
                        Err(_) => break, // Broadcast channel closed
                    };

                    let mut ctx = PeerHandlerContext {
                        framed: &mut framed,
                        task: &task,
                        meta_id,
                        sub_id,
                        storage_client: storage_client.clone(),
                        subtask_tx: subtask_tx.clone(),
                        peer_choking: &mut peer_choking,
                    };

                    self.handle_worker_command(cmd, &mut ctx).await?;
                }
                msg_res = framed.next() => {
                    let ctx = PeerHandlerContext {
                        framed: &mut framed,
                        task: &task,
                        meta_id,
                        sub_id,
                        storage_client: storage_client.clone(),
                        subtask_tx: subtask_tx.clone(),
                        peer_choking: &mut peer_choking,
                    };
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

    async fn handle_worker_command(
        &mut self,
        cmd: WorkerCommand,
        ctx: &mut PeerHandlerContext<'_, MseStream<TcpStream>>,
    ) -> Result<()> {
        let peer_addr = self.peer_addr.clone();
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
                    self.trigger_request(ctx).await?;
                }
            }
            WorkerCommand::RequestPiece(piece_idx) => {
                if self.current_piece.is_none() {
                    debug!(addr = %peer_addr, %piece_idx, "Received forced piece request (Endgame)");
                    self.current_piece = Some(piece_idx);
                    self.is_endgame = false;
                    self.bytes_received = 0;
                    self.bytes_requested = 0;
                    let state_clone = ctx.task.state.clone();
                    let guard = crate::piece_picker::PieceGuard::new(piece_idx, move |idx| {
                        tokio::spawn(async move {
                            if let Some(picker) = state_clone.picker.lock().await.as_mut() {
                                picker.release_piece(idx);
                            }
                        });
                    });
                    self.active_guard = Some(guard);
                    self.trigger_request(ctx).await?;
                }
            }
            WorkerCommand::EndgameFetch(piece_idx) => {
                if self.current_piece.is_none() {
                    debug!(addr = %peer_addr, %piece_idx, "Received forced piece request (EndgameFetch)");
                    self.current_piece = Some(piece_idx);
                    self.is_endgame = true;
                    self.bytes_received = 0;
                    self.bytes_requested = 0;
                    let state_clone = ctx.task.state.clone();
                    let guard = crate::piece_picker::PieceGuard::new(piece_idx, move |idx| {
                        tokio::spawn(async move {
                            if let Some(picker) = state_clone.picker.lock().await.as_mut() {
                                picker.release_piece(idx);
                            }
                        });
                    });
                    self.active_guard = Some(guard);
                    self.trigger_request(ctx).await?;
                } else if self.current_piece == Some(piece_idx) && !self.is_endgame {
                    debug!(addr = %peer_addr, %piece_idx, "Promoting current piece request to EndgameFetch");
                    self.is_endgame = true;
                    self.trigger_request(ctx).await?;
                }
            }
            WorkerCommand::CheckWork => {
                if self.current_piece.is_none() && !*ctx.peer_choking {
                    self.trigger_request(ctx).await?;
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
                let is_private = if let Some(ref torrent) = *ctx.task.state.torrent.lock().await {
                    torrent.is_private()
                } else {
                    false
                };
                if !self.pex_enabled || is_private {
                    return Ok(());
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
                        let pex_msg = crate::worker::bittorrent::protocol::PexMessage::encode_peers(
                            &added, &dropped,
                        );
                        if let Ok(payload) = serde_bencode::to_bytes(&pex_msg) {
                            let _ = ctx
                                .framed
                                .send(PeerMessage::Extended {
                                    id: pex_id,
                                    payload: payload.into(),
                                })
                                .await;
                        }
                        self.last_sent_pex_peers = active_peers;
                    }
                }
            }
        }
        Ok(())
    }
}
