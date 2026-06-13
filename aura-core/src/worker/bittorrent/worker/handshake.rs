use super::types::BtWorker;
use crate::worker::bittorrent::protocol::mse::MseStream;
use crate::worker::bittorrent::protocol::{Handshake, HANDSHAKE_LEN};
use crate::{Error, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::debug;

impl BtWorker {
    pub(crate) async fn connect_and_handshake(
        &self,
    ) -> Result<(MseStream<TcpStream>, [u8; 20], bool)> {
        debug!(addr = %self.peer_addr, "Connecting to peer...");
        let remote_addr: std::net::SocketAddr = self.peer_addr.parse().map_err(|e| {
            Error::Protocol(format!("Invalid peer address {}: {}", self.peer_addr, e))
        })?;

        let connect_fresh = || async {
            timeout(
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
            .map_err(|_| Error::Protocol("Peer connection timeout".to_string()))?
        };

        let handshake_payload =
            Handshake::new(self.info_hash.for_handshake(), self.my_id).serialize();

        // Check if encryption is disabled
        if self.encryption == crate::config::EncryptionPolicy::Disable {
            let stream = connect_fresh().await?;
            let mut mse_stream = MseStream::new(stream);
            debug!(addr = %self.peer_addr, "Sending standard plaintext handshake...");
            timeout(
                std::time::Duration::from_secs(self.connect_timeout_secs),
                async {
                    mse_stream.write_all(&handshake_payload).await?;
                    let mut buf = [0u8; HANDSHAKE_LEN];
                    mse_stream.read_exact(&mut buf).await?;
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
                    mse_stream,
                    res_handshake.peer_id,
                    res_handshake.extension_protocol,
                ))
            })
        } else {
            // Require or Prefer
            let stream = connect_fresh().await?;
            let mut mse_stream = MseStream::new(stream);
            debug!(addr = %self.peer_addr, "Attempting MSE handshake...");

            let mse_res = timeout(
                std::time::Duration::from_secs(self.connect_timeout_secs),
                mse_stream.handshake_outgoing(
                    &self.info_hash.for_handshake(),
                    self.encryption,
                    &handshake_payload,
                ),
            )
            .await;

            match mse_res {
                Ok(Ok(())) => {
                    debug!(
                        addr = %self.peer_addr,
                        "MSE handshake succeeded. Reading peer handshake..."
                    );
                    let res = timeout(
                        std::time::Duration::from_secs(self.connect_timeout_secs),
                        async {
                            let mut buf = [0u8; HANDSHAKE_LEN];
                            mse_stream.read_exact(&mut buf).await?;
                            Ok::<[u8; HANDSHAKE_LEN], std::io::Error>(buf)
                        },
                    )
                    .await;

                    match res {
                        Ok(Ok(buf)) => {
                            let res_handshake = Handshake::deserialize(&buf)?;
                            if res_handshake.info_hash != self.info_hash.for_handshake() {
                                return Err(Error::Protocol(
                                    "Handshake info_hash mismatch".to_string(),
                                ));
                            }
                            Ok((
                                mse_stream,
                                res_handshake.peer_id,
                                res_handshake.extension_protocol,
                            ))
                        }
                        _ => {
                            if self.encryption == crate::config::EncryptionPolicy::Require {
                                return Err(Error::Protocol(
                                    "Failed to read peer handshake under required encryption"
                                        .to_string(),
                                ));
                            }
                            // Fallback to plaintext
                            debug!(
                                addr = %self.peer_addr,
                                "Failed to read peer handshake after MSE. Falling back to plaintext..."
                            );
                            Self::connect_plaintext_fallback(self, remote_addr, &handshake_payload)
                                .await
                        }
                    }
                }
                _ => {
                    if self.encryption == crate::config::EncryptionPolicy::Require {
                        return Err(Error::Protocol(
                            "MSE handshake failed under required encryption".to_string(),
                        ));
                    }
                    // Fallback to plaintext
                    debug!(
                        addr = %self.peer_addr,
                        "MSE handshake failed. Falling back to plaintext..."
                    );
                    Self::connect_plaintext_fallback(self, remote_addr, &handshake_payload).await
                }
            }
        }
    }

    async fn connect_plaintext_fallback(
        &self,
        remote_addr: std::net::SocketAddr,
        handshake_payload: &[u8],
    ) -> Result<(MseStream<TcpStream>, [u8; 20], bool)> {
        let stream = timeout(
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

        let mut mse_stream = MseStream::new(stream);
        timeout(
            std::time::Duration::from_secs(self.connect_timeout_secs),
            async {
                mse_stream.write_all(handshake_payload).await?;
                let mut buf = [0u8; HANDSHAKE_LEN];
                mse_stream.read_exact(&mut buf).await?;
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
                mse_stream,
                res_handshake.peer_id,
                res_handshake.extension_protocol,
            ))
        })
    }
}
