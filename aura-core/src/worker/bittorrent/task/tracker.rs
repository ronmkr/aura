use crate::orchestrator::SubTaskEvent;
use crate::tracker::TrackerClient;
use crate::worker::bittorrent::task::BtTask;
use crate::Result;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

/// Arguments for the BitTorrent tracker background loop.
pub struct TrackerLoopArgs {
    pub my_id: [u8; 20],
    pub port: u16,
    pub token: CancellationToken,
    pub local_addr: Option<std::net::IpAddr>,
    pub user_agent: Option<String>,
    pub proxy: Option<String>,
    pub subtask_tx: tokio::sync::mpsc::Sender<SubTaskEvent>,
}

impl BtTask {
    pub async fn run_tracker_loop(&self, args: TrackerLoopArgs) -> Result<()> {
        let TrackerLoopArgs {
            my_id,
            port,
            token,
            local_addr,
            user_agent,
            proxy,
            subtask_tx,
        } = args;

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
                                if added > 0 {
                                    let _ = subtask_tx.send(SubTaskEvent::PeersDiscovered(self.id)).await;
                                }
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
