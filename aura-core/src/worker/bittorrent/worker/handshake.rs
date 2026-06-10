use super::types::BtWorker;
use crate::worker::bittorrent::protocol::{Handshake, HANDSHAKE_LEN};
use crate::{Error, Result};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::debug;

impl BtWorker {
    pub(crate) async fn connect_and_handshake(&self) -> Result<(TcpStream, [u8; 20], bool)> {
        debug!(addr = %self.peer_addr, "Connecting to peer...");
        let remote_addr: std::net::SocketAddr = self.peer_addr.parse().map_err(|e| {
            Error::Protocol(format!("Invalid peer address {}: {}", self.peer_addr, e))
        })?;

        let mut stream = timeout(
            std::time::Duration::from_secs(self.connect_timeout_secs),
            crate::net_util::connect_tcp_bound(
                remote_addr,
                None,
                self.local_addr,
                self.proxy.as_deref(),
                self.happy_eyeballs_stagger_ms,
            ),
        )
        .await
        .map_err(|_| Error::Protocol("Peer connection timeout".to_string()))??;

        debug!(addr = %self.peer_addr, "Sending handshake...");
        let handshake = Handshake::new(self.info_hash.for_handshake(), self.my_id);

        timeout(
            std::time::Duration::from_secs(self.connect_timeout_secs),
            async {
                stream.write_all(&handshake.serialize()).await?;
                let mut buf = [0u8; HANDSHAKE_LEN];
                use tokio::io::AsyncReadExt;
                stream.read_exact(&mut buf).await?;
                Ok::<[u8; HANDSHAKE_LEN], std::io::Error>(buf)
            },
        )
        .await
        .map_err(|_| Error::Protocol("Peer handshake timeout".to_string()))?
        .map_err(|e| Error::Protocol(format!("Peer handshake error: {}", e)))
        .and_then(|buf| {
            let res_handshake = Handshake::deserialize(&buf)?;

            if res_handshake.info_hash != self.info_hash.for_handshake() {
                return Err(Error::Protocol("Handshake info_hash mismatch".to_string()));
            }

            Ok((
                stream,
                res_handshake.peer_id,
                res_handshake.extension_protocol,
            ))
        })
    }
}
