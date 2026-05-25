pub mod handlers;
pub mod queries;

use crate::dht::protocol::KrpcMessage;
use crate::dht::routing::{NodeId, RoutingTable};
use crate::{Error, InfoHash, Result};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info};

use sha2::{Digest, Sha256};

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

pub struct DhtSecrets {
    pub current: [u8; 32],
    pub previous: [u8; 32],
    pub last_rotation: std::time::Instant,
}

pub struct DhtActor {
    pub(crate) my_id: NodeId,
    pub(crate) socket: Arc<UdpSocket>,
    pub(crate) routing_table: Arc<Mutex<RoutingTable>>,
    pub(crate) command_rx: mpsc::Receiver<DhtCommand>,
    // Map transaction_id -> sender for replies
    pub(crate) pending_queries: Arc<Mutex<BTreeMap<Vec<u8>, mpsc::Sender<KrpcMessage>>>>,
    // info_hash -> Vec<PeerAddr>
    pub(crate) peers: Arc<Mutex<BTreeMap<InfoHash, Vec<SocketAddr>>>>,
    // RemoteAddr -> Token (for announce_peer)
    pub(crate) tokens: Arc<Mutex<BTreeMap<SocketAddr, Vec<u8>>>>,
    pub(crate) db: Option<sled::Db>,
    pub(crate) secrets: Arc<Mutex<DhtSecrets>>,
}

impl DhtActor {
    pub async fn new(
        _addr: &str,
        my_id: NodeId,
        command_rx: mpsc::Receiver<DhtCommand>,
        local_addr: Option<std::net::IpAddr>,
        port: u16,
        db: Option<sled::Db>,
    ) -> Result<Self> {
        let socket = crate::net_util::bind_udp_bound(port, None, local_addr)
            .await
            .map_err(|e| Error::Config(format!("Failed to bind DHT UDP socket: {}", e)))?;
        let current = rand::random::<[u8; 32]>();
        let previous = rand::random::<[u8; 32]>();
        let last_rotation = std::time::Instant::now();
        let secrets = Arc::new(Mutex::new(DhtSecrets {
            current,
            previous,
            last_rotation,
        }));
        Ok(Self {
            my_id,
            socket: Arc::new(socket),
            routing_table: Arc::new(Mutex::new(RoutingTable::new(my_id))),
            command_rx,
            pending_queries: Arc::new(Mutex::new(BTreeMap::new())),
            peers: Arc::new(Mutex::new(BTreeMap::new())),
            tokens: Arc::new(Mutex::new(BTreeMap::new())),
            db,
            secrets,
        })
    }

    pub async fn generate_token(&self, addr: SocketAddr) -> Vec<u8> {
        let mut secrets = self.secrets.lock().await;
        if secrets.last_rotation.elapsed() >= std::time::Duration::from_secs(600) {
            secrets.previous = secrets.current;
            secrets.current = rand::random::<[u8; 32]>();
            secrets.last_rotation = std::time::Instant::now();
        }

        let mut hasher = Sha256::new();
        hasher.update(addr.ip().to_string().as_bytes());
        hasher.update(&secrets.current);
        hasher.finalize().to_vec()
    }

    pub async fn validate_token(&self, addr: SocketAddr, token: &[u8]) -> bool {
        let mut secrets = self.secrets.lock().await;
        if secrets.last_rotation.elapsed() >= std::time::Duration::from_secs(600) {
            secrets.previous = secrets.current;
            secrets.current = rand::random::<[u8; 32]>();
            secrets.last_rotation = std::time::Instant::now();
        }

        let mut hasher = Sha256::new();
        hasher.update(addr.ip().to_string().as_bytes());
        hasher.update(&secrets.current);
        let current_hash = hasher.finalize();
        if current_hash.as_slice() == token {
            return true;
        }

        let mut hasher = Sha256::new();
        hasher.update(addr.ip().to_string().as_bytes());
        hasher.update(&secrets.previous);
        let previous_hash = hasher.finalize();
        previous_hash.as_slice() == token
    }

    pub async fn run(mut self) -> Result<()> {
        info!("DHT Actor started");

        // Load persisted nodes
        self.load_nodes().await;

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

        let mut save_interval = tokio::time::interval(tokio::time::Duration::from_secs(300));
        let mut ping_interval = tokio::time::interval(tokio::time::Duration::from_secs(600));

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
                _ = save_interval.tick() => {
                    self.save_nodes().await;
                }
                _ = ping_interval.tick() => {
                    self.reping_nodes().await;
                }
            }
        }
    }

    async fn load_nodes(&mut self) {
        if let Some(ref db) = self.db {
            let mut count = 0;
            for (_key, val) in db.scan_prefix(b"dht_node:").flatten() {
                // format: id(20) + port(2) + ip(4 or 16)
                if val.len() >= 26 {
                    let mut id = [0u8; 20];
                    id.copy_from_slice(&val[0..20]);
                    let port = u16::from_be_bytes([val[20], val[21]]);
                    let ip_data = &val[22..];
                    let addr = if ip_data.len() == 4 {
                        let ip =
                            std::net::Ipv4Addr::new(ip_data[0], ip_data[1], ip_data[2], ip_data[3]);
                        SocketAddr::new(std::net::IpAddr::V4(ip), port)
                    } else if ip_data.len() == 16 {
                        let mut ip_arr = [0u8; 16];
                        ip_arr.copy_from_slice(ip_data);
                        let ip = std::net::Ipv6Addr::from(ip_arr);
                        SocketAddr::new(std::net::IpAddr::V6(ip), port)
                    } else {
                        continue;
                    };

                    let mut rt = self.routing_table.lock().await;
                    rt.insert(crate::dht::routing::Node { id, addr });
                    let _ = self.send_ping(addr).await;
                    count += 1;
                }
            }
            if count > 0 {
                info!("Loaded {} DHT nodes from database", count);
            }
        }
    }

    async fn save_nodes(&self) {
        if let Some(ref db) = self.db {
            let rt = self.routing_table.lock().await;
            let mut count = 0;
            for bucket in &rt.buckets {
                for node in &bucket.nodes {
                    let mut key = b"dht_node:".to_vec();
                    key.extend_from_slice(&node.id);

                    let mut val = node.id.to_vec();
                    val.extend_from_slice(&node.addr.port().to_be_bytes());
                    match node.addr.ip() {
                        std::net::IpAddr::V4(ip) => val.extend_from_slice(&ip.octets()),
                        std::net::IpAddr::V6(ip) => val.extend_from_slice(&ip.octets()),
                    }

                    let _ = db.insert(key, val);
                    count += 1;
                }
            }
            let _ = db.flush();
            debug!("Saved {} DHT nodes to database", count);
        }
    }

    async fn reping_nodes(&self) {
        let nodes = {
            let rt = self.routing_table.lock().await;
            let mut nodes = Vec::new();
            for bucket in &rt.buckets {
                for node in &bucket.nodes {
                    nodes.push(node.addr);
                }
            }
            nodes
        };

        for addr in nodes {
            let _ = self.send_ping(addr).await;
        }
    }
}
