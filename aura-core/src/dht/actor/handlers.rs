use super::DhtActor;
use crate::dht::protocol::KrpcMessage;
use crate::dht::routing::Node;
use crate::{InfoHash, Result};
use std::collections::BTreeMap;
use std::net::SocketAddr;

impl DhtActor {
    pub(crate) async fn handle_message(&self, msg: KrpcMessage, addr: SocketAddr) -> Result<()> {
        match msg.msg_type.as_str() {
            "q" => {
                let query = msg.query.as_deref().unwrap_or("");
                let args = msg.args.as_ref();

                match query {
                    "ping" => {
                        let mut r = BTreeMap::new();
                        r.insert(
                            "id".to_string(),
                            serde_bencode::value::Value::Bytes(self.my_id.to_vec()),
                        );
                        self.send_response(msg.transaction_id, r, addr).await?;
                    }
                    "get_peers" => {
                        let info_hash = if let Some(serde_bencode::value::Value::Bytes(b)) =
                            args.and_then(|a| a.get("info_hash"))
                        {
                            if b.len() == 20 {
                                let mut h = [0u8; 20];
                                h.copy_from_slice(b);
                                Some(InfoHash::V1(h))
                            } else if b.len() == 32 {
                                let mut h = [0u8; 32];
                                h.copy_from_slice(b);
                                Some(InfoHash::V2(h))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        if let Some(h) = info_hash {
                            let mut r = BTreeMap::new();
                            r.insert(
                                "id".to_string(),
                                serde_bencode::value::Value::Bytes(self.my_id.to_vec()),
                            );

                            // Generate a token
                            let token = vec![1, 2, 3, 4];
                            self.tokens.lock().await.insert(addr, token.clone());
                            r.insert(
                                "token".to_string(),
                                serde_bencode::value::Value::Bytes(token),
                            );

                            let peers_guard = self.peers.lock().await;
                            if let Some(p) = peers_guard.get(&h) {
                                let mut compact = Vec::new();
                                for peer_addr in p {
                                    compact.push(serde_bencode::value::Value::Bytes(
                                        crate::dht::protocol::compact_peer(peer_addr),
                                    ));
                                }
                                r.insert(
                                    "values".to_string(),
                                    serde_bencode::value::Value::List(compact),
                                );
                            } else {
                                let rt = self.routing_table.lock().await;
                                let closest = rt.get_closest_nodes(&h.for_handshake(), 8);
                                r.insert(
                                    "nodes".to_string(),
                                    serde_bencode::value::Value::Bytes(
                                        crate::dht::protocol::compact_nodes(&closest),
                                    ),
                                );
                            }
                            self.send_response(msg.transaction_id, r, addr).await?;
                        }
                    }
                    "announce_peer" => {
                        if let Some(serde_bencode::value::Value::Bytes(b)) =
                            args.and_then(|a| a.get("info_hash"))
                        {
                            let info_hash = if b.len() == 20 {
                                let mut h = [0u8; 20];
                                h.copy_from_slice(b);
                                Some(InfoHash::V1(h))
                            } else if b.len() == 32 {
                                let mut h = [0u8; 32];
                                h.copy_from_slice(b);
                                Some(InfoHash::V2(h))
                            } else {
                                None
                            };

                            if let Some(h) = info_hash {
                                let port = if let Some(serde_bencode::value::Value::Int(p)) =
                                    args.and_then(|a| a.get("port"))
                                {
                                    *p as u16
                                } else {
                                    addr.port()
                                };

                                let peer_addr = SocketAddr::new(addr.ip(), port);
                                let mut peers_guard = self.peers.lock().await;
                                peers_guard.entry(h).or_default().push(peer_addr);

                                let mut r = BTreeMap::new();
                                r.insert(
                                    "id".to_string(),
                                    serde_bencode::value::Value::Bytes(self.my_id.to_vec()),
                                );
                                self.send_response(msg.transaction_id, r, addr).await?;
                            }
                        }
                    }
                    "find_node" => {
                        if let Some(serde_bencode::value::Value::Bytes(b)) =
                            args.and_then(|a| a.get("target"))
                        {
                            if b.len() == 20 {
                                let mut target = [0u8; 20];
                                target.copy_from_slice(b);
                                let mut r = BTreeMap::new();
                                r.insert(
                                    "id".to_string(),
                                    serde_bencode::value::Value::Bytes(self.my_id.to_vec()),
                                );
                                let rt = self.routing_table.lock().await;
                                let closest = rt.get_closest_nodes(&target, 8);
                                r.insert(
                                    "nodes".to_string(),
                                    serde_bencode::value::Value::Bytes(
                                        crate::dht::protocol::compact_nodes(&closest),
                                    ),
                                );
                                self.send_response(msg.transaction_id, r, addr).await?;
                            }
                        }
                    }
                    _ => {}
                }
            }
            "r" | "e" => {
                let mut pending = self.pending_queries.lock().await;
                if let Some(tx) = pending.remove(&msg.transaction_id) {
                    let _ = tx.send(msg.clone()).await;
                }

                if let Some(res) = &msg.response {
                    if let Some(serde_bencode::value::Value::Bytes(id_bytes)) = res.get("id") {
                        if id_bytes.len() == 20 {
                            let mut id = [0u8; 20];
                            id.copy_from_slice(id_bytes);
                            let mut rt = self.routing_table.lock().await;
                            rt.insert(Node { id, addr });
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
