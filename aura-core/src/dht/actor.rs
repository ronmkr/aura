use super::protocol::KrpcMessage;
use super::routing::{Node, NodeId, RoutingTable};
use crate::{Error, InfoHash, Result};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use tracing::info;

pub enum DhtCommand {
    GetPeers {
        info_hash: InfoHash,
        reply_tx: mpsc::Sender<Vec<SocketAddr>>,
    },
    Announce {
        info_hash: InfoHash,
        port: u16,
    },
}

pub struct DhtActor {
    my_id: NodeId,
    socket: Arc<UdpSocket>,
    routing_table: Arc<Mutex<RoutingTable>>,
    command_rx: mpsc::Receiver<DhtCommand>,
    // Map transaction_id -> sender for replies
    pending_queries: Arc<Mutex<BTreeMap<Vec<u8>, mpsc::Sender<KrpcMessage>>>>,
    // info_hash -> Vec<PeerAddr>
    peers: Arc<Mutex<BTreeMap<InfoHash, Vec<SocketAddr>>>>,
    // RemoteAddr -> Token (for announce_peer)
    tokens: Arc<Mutex<BTreeMap<SocketAddr, Vec<u8>>>>,
}

impl DhtActor {
    pub async fn new(
        _addr: &str,
        my_id: NodeId,
        command_rx: mpsc::Receiver<DhtCommand>,
        local_addr: Option<std::net::IpAddr>,
        port: u16,
    ) -> Result<Self> {
        let socket = crate::net_util::bind_udp_bound(port, None, local_addr)
            .await
            .map_err(|e| Error::Config(format!("Failed to bind DHT UDP socket: {}", e)))?;
        Ok(Self {
            my_id,
            socket: Arc::new(socket),
            routing_table: Arc::new(Mutex::new(RoutingTable::new(my_id))),
            command_rx,
            pending_queries: Arc::new(Mutex::new(BTreeMap::new())),
            peers: Arc::new(Mutex::new(BTreeMap::new())),
            tokens: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub async fn run(mut self) -> Result<()> {
        info!("DHT Actor started");

        // Bootstrap with some standard routers
        let bootstrap_nodes = vec![
            "router.bittorrent.com:6881",
            "dht.transmissionbt.com:6881",
            "router.utorrent.com:6881",
        ];

        for addr in bootstrap_nodes {
            if let Ok(target) = addr.parse::<SocketAddr>() {
                let _ = self.send_ping(target).await;
            } else if let Ok(mut addrs) = tokio::net::lookup_host(addr).await {
                if let Some(target) = addrs.next() {
                    let _ = self.send_ping(target).await;
                }
            }
        }

        let mut buf = [0u8; 2048];
        loop {
            tokio::select! {
                res = self.socket.recv_from(&mut buf) => {
                    if let Ok((len, addr)) = res {
                        if let Ok(msg) = KrpcMessage::decode(&buf[..len]) {
                            self.handle_message(msg, addr).await?;
                        }
                    }
                }
                Some(cmd) = self.command_rx.recv() => {
                    match cmd {
                        DhtCommand::GetPeers { info_hash, reply_tx } => {
                            self.handle_get_peers(info_hash, reply_tx).await?;
                        }
                        DhtCommand::Announce { info_hash, port } => {
                            self.handle_announce(info_hash, port).await?;
                        }
                    }
                }
            }
        }
    }

    async fn handle_message(&self, msg: KrpcMessage, addr: SocketAddr) -> Result<()> {
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
                                        super::protocol::compact_peer(peer_addr),
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
                                        super::protocol::compact_nodes(&closest),
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
                                        super::protocol::compact_nodes(&closest),
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

    async fn send_response(
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

    async fn send_ping(&self, addr: SocketAddr) -> Result<()> {
        let mut args = BTreeMap::new();
        args.insert(
            "id".to_string(),
            serde_bencode::value::Value::Bytes(self.my_id.to_vec()),
        );

        let _ = self.send_query("ping", args, addr).await;
        Ok(())
    }

    async fn send_query(
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

        tokio::select! {
            res = rx.recv() => {
                res.ok_or_else(|| Error::Protocol("Query channel closed".to_string()))
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                self.pending_queries.lock().await.remove(&tid);
                Err(Error::Protocol("Query timed out".to_string()))
            }
        }
    }

    async fn handle_get_peers(
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
                                    if let Some(peer) = super::protocol::parse_compact_peer(b) {
                                        discovered_peers.push(peer);
                                    }
                                }
                            }
                        }
                        if let Some(serde_bencode::value::Value::Bytes(nodes_bytes)) =
                            res.get("nodes")
                        {
                            let nodes = super::protocol::parse_compact_nodes(nodes_bytes);
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

    async fn handle_announce(&self, info_hash: InfoHash, port: u16) -> Result<()> {
        let rt = self.routing_table.lock().await;
        let closest = rt.get_closest_nodes(&info_hash.for_handshake(), 8);
        drop(rt);

        for node in closest {
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
                serde_bencode::value::Value::Bytes(vec![1, 2, 3, 4]),
            );

            let _ = self.send_query("announce_peer", args, node.addr).await;
        }
        Ok(())
    }
}
