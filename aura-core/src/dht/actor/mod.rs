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
}
