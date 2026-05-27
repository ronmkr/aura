use crate::bt_task::BtTask;
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

        // Initial announcement
        let _ = self
            .lpd_tx
            .send(crate::lpd::LpdCommand::Announce { info_hash, port })
            .await;

        loop {
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
                _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                }
            }
        }
        Ok(())
    }
}
