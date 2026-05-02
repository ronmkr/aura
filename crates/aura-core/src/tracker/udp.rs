use tokio::net::UdpSocket;
use tracing::{debug, warn};
use crate::{Result, Error};
use crate::torrent::Torrent;
use super::{TrackerClient, Peer};

impl TrackerClient {
    pub(crate) async fn announce_udp(&self, url_str: &str, torrent: &Torrent) -> Result<Vec<Peer>> {
        let url = url::Url::parse(url_str)
            .map_err(|e| Error::Protocol(format!("Invalid UDP tracker URL: {}", e)))?;
        let host = url.host_str().ok_or_else(|| Error::Protocol("Missing host in UDP tracker URL".to_string()))?;
        let port = url.port().ok_or_else(|| Error::Protocol("Missing port in UDP tracker URL".to_string()))?;

        let addrs = tokio::net::lookup_host(format!("{}:{}", host, port)).await
            .map_err(|e| Error::Protocol(format!("Failed to resolve UDP tracker {}: {}", host, e)))?;

        let mut last_error = Error::Protocol("All UDP attempts failed".to_string());

        for addr in addrs {
            debug!(url = %url_str, %addr, "Attempting UDP tracker announce");
            
            let socket = match crate::net_util::bind_udp_bound(0, None, self.local_addr).await {
                Ok(s) => s,
                Err(e) => {
                    debug!(%addr, error = %e, "Failed to bind UDP socket");
                    last_error = e;
                    continue;
                }
            };

            if let Err(e) = socket.connect(addr).await {
                debug!(%addr, error = %e, "Failed to connect UDP socket");
                last_error = Error::Protocol(format!("Failed to connect UDP socket: {}", e));
                continue;
            }

            match self.do_udp_announce(&socket, torrent).await {
                Ok(peers) => return Ok(peers),
                Err(e) => {
                    warn!(%addr, error = %e, "UDP announce failed");
                    last_error = e;
                }
            }
        }

        Err(last_error)
    }

    async fn do_udp_announce(&self, socket: &UdpSocket, torrent: &Torrent) -> Result<Vec<Peer>> {
        use rand::Rng;
        let transaction_id: u32 = rand::thread_rng().gen();
        let connection_id: u64 = 0x41727101980; // Protocol ID

        // 1. Connect Request
        let mut connect_req = Vec::with_capacity(16);
        connect_req.extend_from_slice(&connection_id.to_be_bytes());
        connect_req.extend_from_slice(&0u32.to_be_bytes()); // Action 0: connect
        connect_req.extend_from_slice(&transaction_id.to_be_bytes());

        socket.send(&connect_req).await
            .map_err(|e| Error::Protocol(format!("Failed to send UDP connect request: {}", e)))?;

        let mut buf = [0u8; 2048];
        let len = tokio::time::timeout(std::time::Duration::from_secs(5), socket.recv(&mut buf)).await
            .map_err(|_| Error::Protocol("UDP connect timeout".to_string()))?
            .map_err(|e| Error::Protocol(format!("UDP recv error: {}", e)))?;

        if len < 16 {
            return Err(Error::Protocol("UDP connect response too short".to_string()));
        }

        let action = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let res_tid = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let connection_id = u64::from_be_bytes([buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15]]);

        if action != 0 || res_tid != transaction_id {
            return Err(Error::Protocol("Invalid UDP connect response".to_string()));
        }

        // 2. Announce Request
        let info_hash = torrent.info_hash()?;
        let mut announce_req = Vec::with_capacity(98);
        announce_req.extend_from_slice(&connection_id.to_be_bytes());
        announce_req.extend_from_slice(&1u32.to_be_bytes()); // Action 1: announce
        announce_req.extend_from_slice(&transaction_id.to_be_bytes());
        announce_req.extend_from_slice(&info_hash);
        announce_req.extend_from_slice(&self.peer_id);
        announce_req.extend_from_slice(&0u64.to_be_bytes()); // downloaded
        announce_req.extend_from_slice(&torrent.total_length().to_be_bytes()); // left
        announce_req.extend_from_slice(&0u64.to_be_bytes()); // uploaded
        announce_req.extend_from_slice(&0u32.to_be_bytes()); // event 0: none
        announce_req.extend_from_slice(&0u32.to_be_bytes()); // ip 0: default
        announce_req.extend_from_slice(&rand::thread_rng().gen::<u32>().to_be_bytes()); // key
        announce_req.extend_from_slice(&(-1i32).to_be_bytes()); // num_want -1: default
        announce_req.extend_from_slice(&(self.port).to_be_bytes());

        socket.send(&announce_req).await
            .map_err(|e| Error::Protocol(format!("Failed to send UDP announce request: {}", e)))?;

        let len = tokio::time::timeout(std::time::Duration::from_secs(5), socket.recv(&mut buf)).await
            .map_err(|_| Error::Protocol("UDP announce timeout".to_string()))?
            .map_err(|e| Error::Protocol(format!("UDP recv error: {}", e)))?;

        if len < 20 {
            return Err(Error::Protocol("UDP announce response too short".to_string()));
        }

        let action = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        if action != 1 {
            return Err(Error::Protocol("Invalid UDP announce response action".to_string()));
        }

        // Parse peers from response (starting at offset 20)
        self.parse_compact_peers_raw(&buf[20..len])
    }
}
