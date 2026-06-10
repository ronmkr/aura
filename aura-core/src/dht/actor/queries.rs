use super::DhtActor;
use crate::dht::protocol::KrpcMessage;
use crate::dht::routing::Node;
use crate::{Error, InfoHash, Result};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use tokio::sync::mpsc;

impl DhtActor {
    pub(crate) async fn send_response(
        &self,
        tid: Vec<u8>,
        r: BTreeMap<String, serde_bencode::value::Value>,
        addr: SocketAddr,
    ) -> Result<()> {
        let reply = KrpcMessage {
            transaction_id: tid,
            msg_type: "r".to_string(),
            query: None,
            args: None,
            response: Some(r),
            error: None,
        };
        let _ = self.socket.send_to(&reply.encode()?, addr).await;
        Ok(())
    }

    pub(crate) async fn send_error(
        &self,
        tid: Vec<u8>,
        code: u32,
        message: &str,
        addr: SocketAddr,
    ) -> Result<()> {
        let reply = KrpcMessage {
            transaction_id: tid,
            msg_type: "e".to_string(),
            query: None,
            args: None,
            response: None,
            error: Some((code, message.to_string())),
        };
        let _ = self.socket.send_to(&reply.encode()?, addr).await;
        Ok(())
    }

    pub(crate) async fn send_ping(&self, addr: SocketAddr) -> Result<()> {
        let mut args = BTreeMap::new();
        args.insert(
            "id".to_string(),
            serde_bencode::value::Value::Bytes(self.my_id.to_vec()),
        );

        let _ = self.send_query("ping", args, addr).await;
        Ok(())
    }

    pub(crate) async fn send_query(
        &self,
        query: &str,
        args: BTreeMap<String, serde_bencode::value::Value>,
        addr: SocketAddr,
    ) -> Result<KrpcMessage> {
        let tid: Vec<u8> = (0..4).map(|_| rand::random::<u8>()).collect();
        let (tx, mut rx) = mpsc::channel(1);

        {
            let mut pending = self.pending_queries.lock().await;
            pending.insert(tid.clone(), tx);
        }

        let msg = KrpcMessage {
            transaction_id: tid.clone(),
            msg_type: "q".to_string(),
            query: Some(query.to_string()),
            args: Some(args),
            response: None,
            error: None,
        };

        self.socket.send_to(&msg.encode()?, addr).await?;

        let dht_query_timeout_secs = self.config.load().bittorrent.dht_query_timeout_secs;
        tokio::select! {
            res = rx.recv() => {
                res.ok_or_else(|| Error::Protocol("Query channel closed".to_string()))
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(dht_query_timeout_secs)) => {
                self.pending_queries.lock().await.remove(&tid);
                Err(Error::Protocol("Query timed out".to_string()))
            }
        }
    }

    pub(crate) async fn handle_get_peers(
        &self,
        info_hash: InfoHash,
        reply_tx: mpsc::Sender<Vec<SocketAddr>>,
    ) -> Result<()> {
        let mut closest_nodes = {
            let rt = self.routing_table.lock().await;
            rt.get_closest_nodes(&info_hash.for_handshake(), 8)
        };

        let mut discovered_peers = Vec::new();
        let mut queried = std::collections::HashSet::new();

        for _ in 0..3 {
            let to_query: Vec<Node> = closest_nodes
                .iter()
                .filter(|n| queried.insert(n.addr))
                .take(3)
                .cloned()
                .collect();

            if to_query.is_empty() {
                break;
            }

            let mut new_nodes = Vec::new();
            for node in to_query {
                let mut args = BTreeMap::new();
                args.insert(
                    "id".to_string(),
                    serde_bencode::value::Value::Bytes(self.my_id.to_vec()),
                );
                args.insert(
                    "info_hash".to_string(),
                    serde_bencode::value::Value::Bytes(info_hash.to_vec()),
                );

                if let Ok(reply) = self.send_query("get_peers", args, node.addr).await {
                    if let Some(res) = reply.response {
                        if let Some(serde_bencode::value::Value::List(values)) = res.get("values") {
                            for val in values {
                                if let serde_bencode::value::Value::Bytes(b) = val {
                                    if let Some(peer) = crate::dht::protocol::parse_compact_peer(b)
                                    {
                                        discovered_peers.push(peer);
                                    }
                                }
                            }
                        }
                        if let Some(serde_bencode::value::Value::Bytes(nodes_bytes)) =
                            res.get("nodes")
                        {
                            let nodes = crate::dht::protocol::parse_compact_nodes(nodes_bytes);
                            new_nodes.extend(nodes);
                        }
                    }
                }
            }

            if !discovered_peers.is_empty() {
                break;
            }

            for node in new_nodes {
                if !closest_nodes.iter().any(|n| n.id == node.id) {
                    closest_nodes.push(node);
                }
            }
            closest_nodes.sort_by_key(|n| self.routing_table.blocking_lock().distance(&n.id));
            closest_nodes.truncate(8);
        }

        let _ = reply_tx.send(discovered_peers).await;
        Ok(())
    }

    pub(crate) async fn handle_announce(&self, info_hash: InfoHash, port: u16) -> Result<()> {
        let rt = self.routing_table.lock().await;
        let closest = rt.get_closest_nodes(&info_hash.for_handshake(), 8);
        drop(rt);

        for node in closest {
            let mut gp_args = BTreeMap::new();
            gp_args.insert(
                "id".to_string(),
                serde_bencode::value::Value::Bytes(self.my_id.to_vec()),
            );
            gp_args.insert(
                "info_hash".to_string(),
                serde_bencode::value::Value::Bytes(info_hash.to_vec()),
            );

            let token = if let Ok(reply) = self.send_query("get_peers", gp_args, node.addr).await {
                if let Some(res) = reply.response {
                    if let Some(serde_bencode::value::Value::Bytes(token_bytes)) = res.get("token")
                    {
                        token_bytes.clone()
                    } else {
                        self.generate_token(node.addr).await
                    }
                } else {
                    self.generate_token(node.addr).await
                }
            } else {
                self.generate_token(node.addr).await
            };

            let mut args = BTreeMap::new();
            args.insert(
                "id".to_string(),
                serde_bencode::value::Value::Bytes(self.my_id.to_vec()),
            );
            args.insert(
                "info_hash".to_string(),
                serde_bencode::value::Value::Bytes(info_hash.to_vec()),
            );
            args.insert(
                "port".to_string(),
                serde_bencode::value::Value::Int(port as i64),
            );
            args.insert(
                "token".to_string(),
                serde_bencode::value::Value::Bytes(token),
            );

            let _ = self.send_query("announce_peer", args, node.addr).await;
        }
        Ok(())
    }
}
