use super::super::protocol::{ExtendedHandshake, MetadataMessage, PeerMessage};
use super::super::BtWorker;
use super::PeerHandlerContext;
use crate::orchestrator::SubTaskEvent;
use crate::Result;
use bytes::{Bytes, BytesMut};
use tracing::info;

impl BtWorker {
    pub(crate) async fn handle_basic_messages<S>(
        &mut self,
        msg: PeerMessage,
        ctx: &mut PeerHandlerContext<'_, S>,
    ) -> Result<bool>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        let peer_addr = self.peer_addr.clone();

        match msg {
            PeerMessage::Choke => {
                *ctx.peer_choking = true;
                Ok(true)
            }
            PeerMessage::Unchoke => {
                *ctx.peer_choking = false;
                info!(addr = %peer_addr, "Peer unchoked us; triggering requests");
                self.trigger_request(ctx).await?;
                Ok(true)
            }
            PeerMessage::Bitfield(bits) => {
                let bf = crate::bitfield::Bitfield::from_bytes(
                    &bits,
                    ctx.task
                        .state
                        .pieces_count
                        .load(std::sync::atomic::Ordering::Relaxed),
                );
                ctx.task
                    .update_peer_state(
                        &peer_addr,
                        crate::peer_registry::ConnectionState::Handshaked,
                    )
                    .await;
                let mut picker = ctx.task.state.picker.lock().await;
                if let Some(ref mut p) = *picker {
                    p.add_peer_bitfield(peer_addr.clone(), bf.clone());
                }
                drop(picker);
                info!(meta_id = %ctx.meta_id, addr = %peer_addr, count = bf.count_set(), "Peer bitfield received");
                let _ = ctx
                    .subtask_tx
                    .send(SubTaskEvent::PeerBitfield(ctx.meta_id, self.peer_id, bf))
                    .await;

                if !*ctx.peer_choking {
                    self.trigger_request(ctx).await?;
                }
                Ok(true)
            }
            PeerMessage::Have(idx) => {
                let mut picker = ctx.task.state.picker.lock().await;
                if let Some(ref mut p) = *picker {
                    let mut bf = crate::bitfield::Bitfield::new(p.num_pieces);
                    bf.set(idx as usize, true);
                    p.add_peer_bitfield(peer_addr.clone(), bf);
                    info!(meta_id = %ctx.meta_id, addr = %peer_addr, idx, "Peer reported piece availability");
                }
                drop(picker);
                let _ = ctx
                    .subtask_tx
                    .send(SubTaskEvent::PeerHave(ctx.meta_id, self.peer_id, idx))
                    .await;

                if !*ctx.peer_choking {
                    self.trigger_request(ctx).await?;
                }
                Ok(true)
            }
            PeerMessage::Interested => {
                ctx.task.update_peer_interest(&peer_addr, true).await;
                Ok(true)
            }
            PeerMessage::NotInterested => {
                ctx.task.update_peer_interest(&peer_addr, false).await;
                Ok(true)
            }
            PeerMessage::Extended { id, payload } => {
                if id == 0 {
                    if let Ok(hs) = serde_bencode::from_bytes::<ExtendedHandshake>(&payload) {
                        if let Some(ref m) = hs.m {
                            self.ut_metadata_id = m.get("ut_metadata").copied();
                            self.ut_pex_id = m.get("ut_pex").copied();
                            if let (Some(size), Some(_)) = (hs.metadata_size, self.ut_metadata_id) {
                                self.metadata_buffer = Some(BytesMut::zeroed(size));
                                self.trigger_request(ctx).await?;
                            }
                        }
                    } else if let Err(e) = serde_bencode::from_bytes::<ExtendedHandshake>(&payload)
                    {
                        tracing::warn!(addr = %self.peer_addr, error = %e, "Failed to parse ExtendedHandshake");
                    }
                    Ok(true)
                } else if Some(id) == self.ut_metadata_id {
                    self.handle_metadata_message(payload, ctx).await?;
                    Ok(true)
                } else if Some(id) == self.ut_pex_id {
                    let is_private = if let Some(ref torrent) = *ctx.task.state.torrent.lock().await
                    {
                        torrent.is_private()
                    } else {
                        false
                    };
                    if self.pex_enabled && !is_private {
                        if let Ok(pex_msg) = serde_bencode::from_bytes::<
                            crate::worker::bittorrent::protocol::PexMessage,
                        >(&payload)
                        {
                            let peers = pex_msg.decode_peers();
                            if !peers.is_empty() {
                                let _ = ctx
                                    .subtask_tx
                                    .send(SubTaskEvent::PexPeersDiscovered(self.info_hash, peers))
                                    .await;
                            }
                        }
                    }
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            _ => Ok(false),
        }
    }

    async fn handle_metadata_message<S>(
        &mut self,
        payload: Bytes,
        ctx: &mut PeerHandlerContext<'_, S>,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        if let Some(pos) = payload.windows(2).position(|w| w == b"ee") {
            let bencoded_len = pos + 2;
            let bencoded = &payload[..bencoded_len];
            let data = &payload[bencoded_len..];

            if let Ok(msg) = serde_bencode::from_bytes::<MetadataMessage>(bencoded) {
                if msg.msg_type == 1 {
                    if let Some(ref mut buf) = self.metadata_buffer {
                        let start = msg.piece as usize
                            * crate::worker::bittorrent::protocol::BLOCK_SIZE as usize;
                        if start + data.len() <= buf.len() {
                            buf[start..start + data.len()].copy_from_slice(data);
                            let full_info_dict = buf.clone().freeze();
                            if let Ok(info_val) = serde_bencode::from_bytes::<
                                serde_bencode::value::Value,
                            >(&full_info_dict)
                            {
                                let mut full_torrent_dict = std::collections::HashMap::new();
                                full_torrent_dict.insert(b"info".to_vec(), info_val);
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
                                                let _ = ctx
                                                    .subtask_tx
                                                    .send(SubTaskEvent::MetadataReceived(
                                                        ctx.meta_id,
                                                        ctx.sub_id,
                                                        Box::new(torrent),
                                                    ))
                                                    .await;
                                                self.trigger_request(ctx).await?;
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
        Ok(())
    }
}
