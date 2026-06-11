use super::TrackerClient;
use crate::torrent::Torrent;
use crate::{Error, Result};
use tokio::net::UdpSocket;
use tracing::{debug, warn};

impl TrackerClient {
    pub(crate) async fn scrape_udp(
        &self,
        url_str: &str,
        torrent: &Torrent,
    ) -> Result<(u32, u32, u32)> {
        let timeout_secs = self
            .config
            .as_ref()
            .map(|c| c.load().network.udp_tracker_timeout_secs)
            .unwrap_or(5);
        let timeout_dur = std::time::Duration::from_secs(timeout_secs);

        let url = url::Url::parse(url_str)
            .map_err(|e| Error::Protocol(format!("Invalid UDP tracker URL: {}", e)))?;
        let host = url
            .host_str()
            .ok_or_else(|| Error::Protocol("Missing host in UDP tracker URL".to_string()))?;
        let port = url
            .port()
            .ok_or_else(|| Error::Protocol("Missing port in UDP tracker URL".to_string()))?;

        let addrs = tokio::time::timeout(
            timeout_dur,
            tokio::net::lookup_host(format!("{}:{}", host, port)),
        )
        .await
        .map_err(|_| Error::Protocol(format!("DNS lookup timeout for UDP tracker {}", host)))?
        .map_err(|e| Error::Protocol(format!("Failed to resolve UDP tracker {}: {}", host, e)))?;

        let mut last_error = Error::Protocol("All UDP attempts failed".to_string());

        for addr in addrs {
            debug!(url = %url_str, %addr, "Attempting UDP tracker scrape");

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

            match self.do_udp_scrape(&socket, torrent).await {
                Ok(stats) => return Ok(stats),
                Err(e) => {
                    warn!(%addr, error = %e, "UDP scrape failed");
                    last_error = e;
                }
            }
        }

        Err(last_error)
    }

    async fn do_udp_scrape(
        &self,
        socket: &UdpSocket,
        torrent: &Torrent,
    ) -> Result<(u32, u32, u32)> {
        use rand::RngExt;
        let transaction_id: u32 = rand::rng().random();
        let connection_id: u64 = 0x41727101980; // Protocol ID

        // 1. Connect Request
        let mut connect_req = Vec::with_capacity(16);
        connect_req.extend_from_slice(&connection_id.to_be_bytes());
        connect_req.extend_from_slice(&0u32.to_be_bytes()); // Action 0: connect
        connect_req.extend_from_slice(&transaction_id.to_be_bytes());

        socket
            .send(&connect_req)
            .await
            .map_err(|e| Error::Protocol(format!("Failed to send UDP connect request: {}", e)))?;

        let mut buf = [0u8; 2048];
        let len = tokio::time::timeout(std::time::Duration::from_secs(5), socket.recv(&mut buf))
            .await
            .map_err(|_| Error::Protocol("UDP connect timeout".to_string()))?
            .map_err(|e| Error::Protocol(format!("UDP recv error: {}", e)))?;

        if len < 16 {
            return Err(Error::Protocol(
                "UDP connect response too short".to_string(),
            ));
        }

        let action = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let res_tid = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let connection_id = u64::from_be_bytes([
            buf[8], buf[9], buf[10], buf[11], buf[12], buf[13], buf[14], buf[15],
        ]);

        if action != 0 || res_tid != transaction_id {
            return Err(Error::Protocol("Invalid UDP connect response".to_string()));
        }

        // 2. Scrape Request
        let info_hash = if let Some(h2) = torrent.info_hash_v2()? {
            let mut truncated = [0u8; 20];
            truncated.copy_from_slice(&h2[..20]);
            truncated
        } else {
            torrent
                .info_hash_v1()?
                .ok_or_else(|| Error::Protocol("No info hash available".to_string()))?
        };

        let mut scrape_req = Vec::with_capacity(36);
        scrape_req.extend_from_slice(&connection_id.to_be_bytes());
        scrape_req.extend_from_slice(&2u32.to_be_bytes()); // Action 2: scrape
        scrape_req.extend_from_slice(&transaction_id.to_be_bytes());
        scrape_req.extend_from_slice(&info_hash);

        socket
            .send(&scrape_req)
            .await
            .map_err(|e| Error::Protocol(format!("Failed to send UDP scrape request: {}", e)))?;

        let len = tokio::time::timeout(std::time::Duration::from_secs(5), socket.recv(&mut buf))
            .await
            .map_err(|_| Error::Protocol("UDP scrape timeout".to_string()))?
            .map_err(|e| Error::Protocol(format!("UDP recv error: {}", e)))?;

        if len < 20 {
            return Err(Error::Protocol("UDP scrape response too short".to_string()));
        }

        let action = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]);
        let res_tid = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);

        if action != 2 || res_tid != transaction_id {
            return Err(Error::Protocol(
                "Invalid UDP scrape response action/transaction_id".to_string(),
            ));
        }

        let complete = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
        let downloaded = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);
        let incomplete = u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]);

        Ok((complete, incomplete, downloaded))
    }
}
