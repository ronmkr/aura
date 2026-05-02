//! dht: Implementation of the BitTorrent DHT (Kademlia).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, debug, warn};
use crate::{Result, Error};

pub type NodeId = [u8; 20];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    pub id: NodeId,
    pub addr: SocketAddr,
}

/// A K-Bucket stores up to 8 nodes (standard K value).
pub struct KBucket {
    nodes: Vec<Node>,
}

impl KBucket {
    pub fn new() -> Self {
        Self { nodes: Vec::with_capacity(8) }
    }

    pub fn insert(&mut self, node: Node) {
        if let Some(pos) = self.nodes.iter().position(|n| n.id == node.id) {
            self.nodes.remove(pos);
        }
        if self.nodes.len() < 8 {
            self.nodes.push(node);
        }
    }
}

impl Default for KBucket {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrpcMessage {
    #[serde(rename = "t")]
    pub transaction_id: Vec<u8>,
    #[serde(rename = "y")]
    pub msg_type: String, // "q", "r", or "e"
    #[serde(rename = "q")]
    pub query: Option<String>,
    #[serde(rename = "a")]
    pub args: Option<BTreeMap<String, serde_bencode::value::Value>>,
    #[serde(rename = "r")]
    pub response: Option<BTreeMap<String, serde_bencode::value::Value>>,
    #[serde(rename = "e")]
    pub error: Option<(i32, String)>,
}

impl KrpcMessage {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        serde_bencode::to_bytes(self)
            .map_err(|e| Error::Protocol(format!("Failed to serialize DHT msg: {}", e)))
    }

    pub fn ping(tid: Vec<u8>, my_id: NodeId) -> Self {
        let mut args = BTreeMap::new();
        args.insert("id".to_string(), serde_bencode::value::Value::Bytes(my_id.to_vec()));
        Self {
            transaction_id: tid,
            msg_type: "q".to_string(),
            query: Some("ping".to_string()),
            args: Some(args),
            response: None,
            error: None,
        }
    }

    pub fn find_node(tid: Vec<u8>, my_id: NodeId, target: NodeId) -> Self {
        let mut args = BTreeMap::new();
        args.insert("id".to_string(), serde_bencode::value::Value::Bytes(my_id.to_vec()));
        args.insert("target".to_string(), serde_bencode::value::Value::Bytes(target.to_vec()));
        Self {
            transaction_id: tid,
            msg_type: "q".to_string(),
            query: Some("find_node".to_string()),
            args: Some(args),
            response: None,
            error: None,
        }
    }

    pub fn get_peers(tid: Vec<u8>, my_id: NodeId, info_hash: [u8; 20]) -> Self {
        let mut args = BTreeMap::new();
        args.insert("id".to_string(), serde_bencode::value::Value::Bytes(my_id.to_vec()));
        args.insert("info_hash".to_string(), serde_bencode::value::Value::Bytes(info_hash.to_vec()));
        Self {
            transaction_id: tid,
            msg_type: "q".to_string(),
            query: Some("get_peers".to_string()),
            args: Some(args),
            response: None,
            error: None,
        }
    }

    pub fn announce_peer(tid: Vec<u8>, my_id: NodeId, info_hash: [u8; 20], port: u16, token: Vec<u8>) -> Self {
        let mut args = BTreeMap::new();
        args.insert("id".to_string(), serde_bencode::value::Value::Bytes(my_id.to_vec()));
        args.insert("info_hash".to_string(), serde_bencode::value::Value::Bytes(info_hash.to_vec()));
        args.insert("port".to_string(), serde_bencode::value::Value::Int(port as i64));
        args.insert("token".to_string(), serde_bencode::value::Value::Bytes(token));
        Self {
            transaction_id: tid,
            msg_type: "q".to_string(),
            query: Some("announce_peer".to_string()),
            args: Some(args),
            response: None,
            error: None,
        }
    }
}

pub struct RoutingTable {
    my_id: NodeId,
    buckets: Vec<KBucket>,
}

impl RoutingTable {
    pub fn new(my_id: NodeId) -> Self {
        let mut buckets = Vec::with_capacity(160);
        for _ in 0..160 {
            buckets.push(KBucket::new());
        }
        Self { my_id, buckets }
    }

    pub fn distance(&self, id: &NodeId) -> u128 {
        let mut dist = 0u128;
        for (a, b) in self.my_id.iter().zip(id.iter()) {
            dist = (dist << 8) | (a ^ b) as u128;
        }
        dist
    }

    pub fn bucket_index(&self, id: &NodeId) -> usize {
        for (i, (a, b)) in self.my_id.iter().zip(id.iter()).enumerate() {
            let xor = a ^ b;
            if xor != 0 {
                return i * 8 + xor.leading_zeros() as usize;
            }
        }
        159
    }

    pub fn insert(&mut self, node: Node) {
        let idx = self.bucket_index(&node.id);
        self.buckets[idx].insert(node);
    }

    pub fn get_closest_nodes(&self, target: &NodeId, k: usize) -> Vec<Node> {
        let mut all_nodes = Vec::new();
        for bucket in &self.buckets {
            all_nodes.extend(bucket.nodes.clone());
        }

        all_nodes.sort_by_key(|n| {
            let mut dist = 0u128;
            for (a, b) in n.id.iter().zip(target.iter()) {
                dist = (dist << 8) | (a ^ b) as u128;
            }
            dist
        });

        all_nodes.truncate(k);
        all_nodes
    }
}

#[derive(Debug)]
pub enum DhtCommand {
    GetPeers {
        info_hash: [u8; 20],
        reply_tx: mpsc::UnboundedSender<Vec<SocketAddr>>,
    },
    Announce {
        info_hash: [u8; 20],
        port: u16,
    },
}

pub struct DhtActor {
    my_id: NodeId,
    socket: Arc<UdpSocket>,
    routing_table: Arc<Mutex<RoutingTable>>,
    command_rx: mpsc::Receiver<DhtCommand>,
    // Map transaction_id -> sender for replies
    pending_queries: Arc<Mutex<BTreeMap<Vec<u8>, mpsc::UnboundedSender<KrpcMessage>>>>,
}

impl DhtActor {
    pub async fn new(_addr: &str, my_id: NodeId, command_rx: mpsc::Receiver<DhtCommand>, local_addr: Option<std::net::IpAddr>, port: u16) -> Result<Self> {
        let socket = crate::net_util::bind_udp_bound(port, None, local_addr).await
            .map_err(|e| Error::Config(format!("Failed to bind DHT UDP socket: {}", e)))?;
        Ok(Self {
            my_id,
            socket: Arc::new(socket),
            routing_table: Arc::new(Mutex::new(RoutingTable::new(my_id))),
            command_rx,
            pending_queries: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub async fn run(mut self) -> Result<()> {
        info!("DHT Actor started");
        
        // Bootstrap
        self.bootstrap().await;

        let socket_recv = self.socket.clone();
        let routing_table_recv = self.routing_table.clone();
        let pending_queries_recv = self.pending_queries.clone();
        let socket_resp = self.socket.clone();
        let my_id = self.my_id;

        // Receiver loop
        tokio::spawn(async move {
            let mut buf = [0u8; 2048];
            loop {
                match socket_recv.recv_from(&mut buf).await {
                    Ok((len, addr)) => {
                        if let Ok(msg) = serde_bencode::from_bytes::<KrpcMessage>(&buf[..len]) {
                            debug!(?addr, type = %msg.msg_type, "DHT msg received");
                            
                            // 1. Update routing table
                            if let Some(args) = &msg.args {
                                if let Some(serde_bencode::value::Value::Bytes(id)) = args.get("id") {
                                    if id.len() == 20 {
                                        let mut node_id = [0u8; 20];
                                        node_id.copy_from_slice(id);
                                        let mut rt = routing_table_recv.lock().await;
                                        rt.insert(Node { id: node_id, addr });
                                    }
                                }
                            }

                            // 2. Handle query or response
                            match msg.msg_type.as_str() {
                                "q" => {
                                    // Respond to queries (minimal implementation for now)
                                    if let Some(query) = &msg.query {
                                        if query == "ping" {
                                            let mut r = BTreeMap::new();
                                            r.insert("id".to_string(), serde_bencode::value::Value::Bytes(my_id.to_vec()));
                                            let resp = KrpcMessage {
                                                transaction_id: msg.transaction_id.clone(),
                                                msg_type: "r".to_string(),
                                                query: None,
                                                args: None,
                                                response: Some(r),
                                                error: None,
                                            };
                                            if let Ok(data) = resp.serialize() {
                                                let _ = socket_resp.send_to(&data, addr).await;
                                            }
                                        }
                                    }
                                }
                                "r" => {
                                    let mut pending = pending_queries_recv.lock().await;
                                    if let Some(tx) = pending.remove(&msg.transaction_id) {
                                        let _ = tx.send(msg);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => warn!("DHT socket recv error: {}", e),
                }
            }
        });

        // Command loop
        while let Some(cmd) = self.command_rx.recv().await {
            match cmd {
                DhtCommand::GetPeers { info_hash, reply_tx } => {
                    self.perform_get_peers(info_hash, reply_tx).await;
                }
                DhtCommand::Announce { info_hash, port } => {
                    self.perform_announce(info_hash, port).await;
                }
            }
        }

        Ok(())
    }

    async fn bootstrap(&self) {
        let bootstrap_nodes = vec![
            "router.bittorrent.com:6881",
            "dht.transmissionbt.com:6881",
            "router.utorrent.com:6881",
        ];

        for node_addr in bootstrap_nodes {
            if let Ok(addrs) = tokio::net::lookup_host(node_addr).await {
                for addr in addrs {
                    let tid = rand::random::<u16>().to_be_bytes().to_vec();
                    let ping = KrpcMessage::ping(tid, self.my_id);
                    if let Ok(data) = ping.serialize() {
                        let _ = self.socket.send_to(&data, addr).await;
                    }
                }
            }
        }
    }

    async fn perform_get_peers(&self, info_hash: [u8; 20], reply_tx: mpsc::UnboundedSender<Vec<SocketAddr>>) {
        let nodes = {
            let rt = self.routing_table.lock().await;
            rt.get_closest_nodes(&info_hash, 8)
        };

        for node in nodes {
            let tid = rand::random::<u16>().to_be_bytes().to_vec();
            let msg = KrpcMessage::get_peers(tid.clone(), self.my_id, info_hash);
            if let Ok(data) = msg.serialize() {
                let (tx, mut rx) = mpsc::unbounded_channel();
                {
                    let mut pending = self.pending_queries.lock().await;
                    pending.insert(tid, tx);
                }

                let socket = self.socket.clone();
                let addr = node.addr;
                let reply_tx_inner = reply_tx.clone();

                tokio::spawn(async move {
                    let _ = socket.send_to(&data, addr).await;
                    if let Ok(Some(resp)) = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
                        if let Some(r) = resp.response {
                            if let Some(serde_bencode::value::Value::List(values)) = r.get("values") {
                                let mut peers = Vec::new();
                                for val in values {
                                    if let serde_bencode::value::Value::Bytes(b) = val {
                                        if b.len() == 6 {
                                            let ip = format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]);
                                            let port = u16::from_be_bytes([b[4], b[5]]);
                                            if let Ok(p_addr) = format!("{}:{}", ip, port).parse::<SocketAddr>() {
                                                peers.push(p_addr);
                                            }
                                        }
                                    }
                                }
                                if !peers.is_empty() {
                                    let _ = reply_tx_inner.send(peers);
                                }
                            }
                        }
                    }
                });
            }
        }
    }

    async fn perform_announce(&self, _info_hash: [u8; 20], _port: u16) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_id_distance() {
        let id1 = [0u8; 20];
        let id2 = [1u8; 20];
        let rt = RoutingTable::new(id1);
        assert!(rt.distance(&id2) > 0);
    }

    #[test]
    fn test_bucket_index() {
        let id1 = [0u8; 20];
        let mut id2 = [0u8; 20];
        let rt = RoutingTable::new(id1);
        
        id2[0] = 0b1000_0000;
        assert_eq!(rt.bucket_index(&id2), 0);
        
        id2[0] = 0b0100_0000;
        assert_eq!(rt.bucket_index(&id2), 1);

        id2[0] = 0;
        id2[1] = 0b1000_0000;
        assert_eq!(rt.bucket_index(&id2), 8);
    }

    #[test]
    fn test_krpc_serialization() {
        let tid = vec![1, 2];
        let my_id = [0u8; 20];
        let msg = KrpcMessage::ping(tid.clone(), my_id);
        let serialized = msg.serialize().unwrap();
        let deserialized: KrpcMessage = serde_bencode::from_bytes(&serialized).unwrap();
        assert_eq!(deserialized.transaction_id, tid);
        assert_eq!(deserialized.msg_type, "q");
    }

    #[test]
    fn test_get_closest_nodes() {
        let my_id = [0u8; 20];
        let mut rt = RoutingTable::new(my_id);
        
        for i in 1..10 {
            let mut id = [0u8; 20];
            id[19] = i as u8;
            rt.insert(Node { id, addr: "127.0.0.1:80".parse().unwrap() });
        }

        let target = [0u8; 20];
        let closest = rt.get_closest_nodes(&target, 5);
        assert_eq!(closest.len(), 5);
    }
}
