use crate::worker::bittorrent::task::BtTask;
use crate::Result;
use tracing::info;

impl BtTask {
    pub async fn run_lpd_loop(
        &self,
        port: u16,
        token: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let info_hash = self.state.info_hash;
        info!(%self.id, "Starting LPD announcement loop");

        let is_private = if let Some(ref torrent) = *self.state.torrent.lock().await {
            torrent.is_private()
        } else {
            false
        };

        if is_private {
            info!(%self.id, "Skipping LPD loop for private torrent");
            return Ok(());
        }

        // Initial announcement
        let _ = self
            .lpd_tx
            .send(crate::lpd::LpdCommand::Announce { info_hash, port })
            .await;

        loop {
            let is_private = if let Some(ref torrent) = *self.state.torrent.lock().await {
                torrent.is_private()
            } else {
                false
            };

            if is_private {
                info!(%self.id, "Stopping LPD announcement loop for private torrent");
                let _ = self
                    .lpd_tx
                    .send(crate::lpd::LpdCommand::Remove { info_hash })
                    .await;
                break;
            }

            let lpd_announce_interval_secs = self
                .state
                .config
                .load()
                .bittorrent
                .lpd_announce_interval_secs;
            tokio::select! {
                _ = token.cancelled() => {
                    let _ = self
                        .lpd_tx
                        .send(crate::lpd::LpdCommand::Remove {
                            info_hash,
                        })
                        .await;
                    break;
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(lpd_announce_interval_secs)) => {
                }
            }
        }
        Ok(())
    }
}
