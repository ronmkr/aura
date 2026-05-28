use crate::tracker::Peer;
use crate::worker::bittorrent::task::BtTask;
use crate::Result;
use tracing::{debug, info, warn};

impl BtTask {
    pub async fn run_dht_loop(&self, token: tokio_util::sync::CancellationToken) -> Result<()> {
        let info_hash = self.state.info_hash;
        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                _ = async {
                    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
                    let cmd = crate::dht::DhtCommand::GetPeers {
                        info_hash,
                        reply_tx: tx,
                    };

                    if let Err(e) = self.dht_tx.send(cmd).await {
                        warn!("Failed to send DHT command: {}", e);
                        return;
                    }

                    if let Some(addrs) = rx.recv().await {
                        let mut peers = Vec::new();
                        for addr in addrs {
                            let ip: std::net::IpAddr = addr.ip();
                            peers.push(Peer {
                                id: None,
                                ip: ip.to_string(),
                                port: addr.port(),
                            });
                        }

                        if !peers.is_empty() {
                            info!(%self.id, count = peers.len(), "Discovered peers from DHT");
                            let mut registry = self.state.registry.lock().await;
                            let added = registry.add_peers(peers);
                            debug!(%self.id, added, "Added unique DHT peers to registry");
                        }
                    }
                } => {}
            }

            tokio::select! {
                _ = token.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(120)) => {}
            }
        }
        Ok(())
    }
}
