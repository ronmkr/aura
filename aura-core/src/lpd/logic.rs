//! lpd: Local Peer Discovery (BEP 14) implementation.

use crate::tracker::Peer;
use crate::{Error, InfoHash, Result};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{debug, info};

const LPD_MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 192, 152, 143);
const LPD_PORT: u16 = 6771;

#[derive(Debug, Clone)]
pub enum LpdCommand {
    Announce { info_hash: InfoHash, port: u16 },
    Remove { info_hash: InfoHash },
}

pub struct LpdActor {
    command_rx: mpsc::Receiver<LpdCommand>,
    event_tx: mpsc::Sender<crate::orchestrator::SubTaskEvent>,
    socket: UdpSocket,
    active_hashes: HashSet<(InfoHash, u16)>, // (info_hash, listen_port)
    cookie: String,
}

impl LpdActor {
    pub async fn new(
        command_rx: mpsc::Receiver<LpdCommand>,
        event_tx: mpsc::Sender<crate::orchestrator::SubTaskEvent>,
        local_addr: Option<IpAddr>,
    ) -> Result<Self> {
        let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, LPD_PORT);

        let std_socket = std::net::UdpSocket::bind(bind_addr)
            .map_err(|e| Error::Config(format!("Failed to bind LPD UDP socket: {}", e)))?;

        std_socket.set_multicast_loop_v4(true).ok();

        let interface = match local_addr {
            Some(IpAddr::V4(v4)) => v4,
            _ => Ipv4Addr::UNSPECIFIED,
        };

        std_socket
            .join_multicast_v4(&LPD_MULTICAST_ADDR, &interface)
            .map_err(|e| Error::Config(format!("Failed to join LPD multicast group: {}", e)))?;

        std_socket.set_nonblocking(true).ok();
        let socket = UdpSocket::from_std(std_socket).map_err(Error::Io)?;

        let cookie = format!("{:08x}", rand::random::<u32>());

        Ok(Self {
            command_rx,
            event_tx,
            socket,
            active_hashes: HashSet::new(),
            cookie,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        info!("LPD Actor started");

        let mut announce_interval = tokio::time::interval(std::time::Duration::from_secs(300));
        let mut buf = [0u8; 1024];

        loop {
            tokio::select! {
                _ = announce_interval.tick() => {
                    self.broadcast_announcements().await;
                }
                Some(cmd) = self.command_rx.recv() => {
                    match cmd {
                        LpdCommand::Announce { info_hash, port } => {
                            self.active_hashes.insert((info_hash, port));
                            self.send_announce(info_hash, port).await;
                        }
                        LpdCommand::Remove { info_hash } => {
                            self.active_hashes.retain(|(h, _)| *h != info_hash);
                        }
                    }
                }
                res = self.socket.recv_from(&mut buf) => {
                    if let Ok((len, addr)) = res {
                        self.handle_packet(&buf[..len], addr).await;
                    }
                }
            }
        }
    }

    async fn broadcast_announcements(&self) {
        for (info_hash, port) in &self.active_hashes {
            self.send_announce(*info_hash, *port).await;
        }
    }

    async fn send_announce(&self, info_hash: InfoHash, port: u16) {
        let info_hash_hex = hex::encode(info_hash.to_vec());
        let message = format!(
            "BT-SEARCH * HTTP/1.1\r
\
             Host: 239.192.152.143:6771\r
\
             Port: {}\r
\
             Infohash: {}\r
\
             cookie: {}\r
\
             \r
\r
",
            port, info_hash_hex, self.cookie
        );

        let dest = SocketAddr::new(IpAddr::V4(LPD_MULTICAST_ADDR), LPD_PORT);
        if let Err(e) = self.socket.send_to(message.as_bytes(), dest).await {
            debug!("Failed to send LPD announce: {}", e);
        }
    }

    async fn handle_packet(&self, data: &[u8], addr: SocketAddr) {
        if let Some((h, peer)) = self.parse_packet(data, addr) {
            let _ = self
                .event_tx
                .send(crate::orchestrator::SubTaskEvent::LpdPeerDiscovered(
                    h, peer,
                ))
                .await;
        }
    }

    fn parse_packet(&self, data: &[u8], addr: SocketAddr) -> Option<(InfoHash, Peer)> {
        let text = String::from_utf8_lossy(data);
        if !text.starts_with("BT-SEARCH") {
            return None;
        }

        let mut port = None;
        let mut info_hash = None;
        let mut remote_cookie = None;

        for line in text.lines() {
            let parts: Vec<&str> = line.splitn(2, ':').map(|s| s.trim()).collect();
            if parts.len() < 2 {
                continue;
            }

            match parts[0].to_lowercase().as_str() {
                "port" => port = parts[1].parse::<u16>().ok(),
                "infohash" => {
                    if let Ok(h) = hex::decode(parts[1]) {
                        if h.len() == 20 {
                            let mut hash = [0u8; 20];
                            hash.copy_from_slice(&h);
                            info_hash = Some(InfoHash::V1(hash));
                        } else if h.len() == 32 {
                            let mut hash = [0u8; 32];
                            hash.copy_from_slice(&h);
                            info_hash = Some(InfoHash::V2(hash));
                        }
                    }
                }
                "cookie" => remote_cookie = Some(parts[1].to_string()),
                _ => {}
            }
        }

        if let (Some(p), Some(h), Some(c)) = (port, info_hash, remote_cookie) {
            if c == self.cookie {
                return None;
            }

            if self
                .active_hashes
                .iter()
                .any(|(active_h, _)| *active_h == h)
            {
                let peer = Peer {
                    id: None,
                    ip: addr.ip().to_string(),
                    port: p,
                };
                return Some((h, peer));
            }
        }
        None
    }
}

#[cfg(test)]
#[cfg(test)]
mod tests {
    use super::*;

    use proptest::prelude::*;
    use tokio::sync::mpsc;

    proptest! {
        #[test]
        fn test_lpd_packet_parsing_proptest(ref s in "\\PC*") {
            let (_event_tx, _event_rx) = mpsc::channel::<crate::orchestrator::SubTaskEvent>(1);

            let text = String::from_utf8_lossy(s.as_bytes());
            for line in text.lines() {
                let _parts: Vec<&str> = line.splitn(2, ':').map(|s| s.trim()).collect();
            }
        }
    }

    #[tokio::test]
    async fn test_lpd_packet_parsing() {
        let (_cmd_tx, _cmd_rx) = mpsc::channel(1);
        let (event_tx, mut event_rx) = mpsc::channel(1);

        let actor = LpdActor {
            command_rx: _cmd_rx,
            event_tx,
            socket: UdpSocket::bind("127.0.0.1:0").await.unwrap(),
            active_hashes: HashSet::new(),
            cookie: "my-cookie".to_string(),
        };

        let info_hash = InfoHash::V1([1u8; 20]);
        let mut actor = actor;
        actor.active_hashes.insert((info_hash, 6881));

        let message = "BT-SEARCH * HTTP/1.1\r
\
                   Port: 6882\r
\
                   Infohash: 0101010101010101010101010101010101010101\r
\
                   cookie: other-cookie\r
\r
";

        let addr = "192.168.1.100:12345".parse::<SocketAddr>().unwrap();
        actor.handle_packet(message.as_bytes(), addr).await;

        let event = event_rx.recv().await.unwrap();
        if let crate::orchestrator::SubTaskEvent::LpdPeerDiscovered(h, peer) = event {
            assert_eq!(h, info_hash);
            assert_eq!(peer.ip, "192.168.1.100");
            assert_eq!(peer.port, 6882);
        } else {
            panic!("Wrong event type");
        }
    }

    #[tokio::test]
    async fn test_lpd_ignore_own_cookie() {
        let (_cmd_tx, _cmd_rx) = mpsc::channel(1);
        let (event_tx, mut event_rx) = mpsc::channel(1);

        let actor = LpdActor {
            command_rx: _cmd_rx,
            event_tx,
            socket: UdpSocket::bind("127.0.0.1:0").await.unwrap(),
            active_hashes: HashSet::new(),
            cookie: "my-cookie".to_string(),
        };

        let info_hash = InfoHash::V1([1u8; 20]);
        let mut actor = actor;
        actor.active_hashes.insert((info_hash, 6881));

        let message = "BT-SEARCH * HTTP/1.1\r
\
                   Port: 6881\r
\
                   Infohash: 0101010101010101010101010101010101010101\r
\
                   cookie: my-cookie\r
\r
";

        let addr = "192.168.1.100:12345".parse::<SocketAddr>().unwrap();
        actor.handle_packet(message.as_bytes(), addr).await;

        tokio::select! {
            _ = event_rx.recv() => panic!("Should not have received an event"),
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
        }
    }
}
