use std::collections::BTreeMap;
use std::sync::Arc;
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use tracing::info;
use crate::{Result, Error};
use self::routing::{NodeId, RoutingTable, Node};
use self::protocol::KrpcMessage;

pub mod routing;
pub mod protocol;

pub enum DhtCommand {
    GetPeers {
        info_hash: [u8; 20],
        reply_tx: mpsc::Sender<Vec<SocketAddr>>,
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
    pending_queries: Arc<Mutex<BTreeMap<Vec<u8>, mpsc::Sender<KrpcMessage>>>>,
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
                // Handle incoming queries
                if let Some(query) = &msg.query {
                    if query == "ping" {
                        let mut r = BTreeMap::new();
                        r.insert("id".to_string(), serde_bencode::value::Value::Bytes(self.my_id.to_vec()));
                        let reply = KrpcMessage {
                            transaction_id: msg.transaction_id,
                            msg_type: "r".to_string(),
                            query: None,
                            args: None,
                            response: Some(r),
                            error: None,
                        };
                        let _ = self.socket.send_to(&reply.encode()?, addr).await;
                    }
                }
            }
            "r" | "e" => {
                // Handle replies to our queries
                let mut pending = self.pending_queries.lock().await;
                if let Some(tx) = pending.remove(&msg.transaction_id) {
                    let _ = tx.send(msg.clone()).await;
                }
                
                // Also update routing table if it's a valid response
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

    async fn send_ping(&self, addr: SocketAddr) -> Result<()> {
        let tid = vec![1, 2]; // Simple static TID for bootstrap
        let mut args = BTreeMap::new();
        args.insert("id".to_string(), serde_bencode::value::Value::Bytes(self.my_id.to_vec()));
        
        let msg = KrpcMessage {
            transaction_id: tid,
            msg_type: "q".to_string(),
            query: Some("ping".to_string()),
            args: Some(args),
            response: None,
            error: None,
        };

        self.socket.send_to(&msg.encode()?, addr).await
            .map_err(|e| Error::Protocol(format!("Failed to send DHT ping: {}", e)))?;
        Ok(())
    }

    async fn handle_get_peers(&self, _info_hash: [u8; 20], _reply_tx: mpsc::Sender<Vec<SocketAddr>>) -> Result<()> {
        // Full iterative find_peers logic would go here
        // For now, we return empty to avoid blocking
        Ok(())
    }

    async fn handle_announce(&self, _info_hash: [u8; 20], _port: u16) -> Result<()> {
        // Announce logic
        Ok(())
    }
}
