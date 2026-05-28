use crate::tracker::TrackerClient;
use crate::worker::bittorrent::task::BtTask;
use crate::Result;
use tracing::{debug, info};

impl BtTask {
    pub async fn run_tracker_loop(
        &self,
        my_id: [u8; 20],
        port: u16,
        token: tokio_util::sync::CancellationToken,
        local_addr: Option<std::net::IpAddr>,
        user_agent: Option<String>,
        proxy: Option<String>,
    ) -> Result<()> {
        let tracker = TrackerClient::new(my_id, port, local_addr, user_agent, proxy);
        info!(%self.id, "Starting tracker announce");

        loop {
            let torrent_opt = self.state.torrent.lock().await.clone();
            if let Some(torrent) = torrent_opt {
                tokio::select! {
                    _ = token.cancelled() => break,
                    res = tracker.announce(&torrent) => {
                        match res {
                            Ok(peers) => {
                                info!(%self.id, count = peers.len(), "Discovered peers from tracker");
                                let mut registry = self.state.registry.lock().await;
                                let added = registry.add_peers(peers);
                                debug!(%self.id, added, "Added unique peers to registry");
                            }
                            Err(e) => {
                                tracing::warn!(%self.id, error = %e, "All tracker announces failed");
                            }
                        }
                    }
                }
            }

            tokio::select! {
                _ = token.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
            }
        }
        Ok(())
    }
}
