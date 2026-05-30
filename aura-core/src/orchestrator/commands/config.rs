use crate::orchestrator::Orchestrator;
use std::sync::Arc;
use tracing::{info, warn};

impl Orchestrator {
    pub(crate) async fn handle_reload_config(
        &mut self,
        new_config: Arc<crate::Config>,
        resp_tx: tokio::sync::oneshot::Sender<()>,
    ) {
        info!("Reloading configuration");
        self.throttler
            .set_global_download_limit(new_config.bandwidth.global_download_limit);
        self.throttler
            .set_global_upload_limit(new_config.bandwidth.global_upload_limit);

        // Update VPN provider if changed
        if self.config.load().vpn != new_config.vpn
            || self.config.load().network.interface != new_config.network.interface
        {
            self.update_vpn_provider(&new_config);
        }

        // Update CredentialProvider if paths changed
        if self.config.load().credentials.netrc_path != new_config.credentials.netrc_path
            || self.config.load().credentials.cookie_file != new_config.credentials.cookie_file
        {
            info!("Reloading credentials");
            let mut new_provider = crate::config::credentials::CredentialProvider::new();
            if let Some(ref netrc) = new_config.credentials.netrc_path {
                if let Err(e) = new_provider.load_netrc(netrc) {
                    warn!("Failed to reload .netrc from {}: {}", netrc, e);
                }
            }
            if let Some(ref cookie_file) = new_config.credentials.cookie_file {
                if let Err(e) = new_provider.load_cookies(cookie_file) {
                    warn!("Failed to reload cookies from {}: {}", cookie_file, e);
                }
            }
            self.credential_provider = Arc::new(new_provider);
        }

        // Update DNS resolver if changed
        if self.config.load().network.dns_resolver != new_config.network.dns_resolver {
            info!("Reloading DNS resolver");
            match crate::net_util::create_resolver(&new_config.network.dns_resolver).await {
                Ok(new_resolver) => {
                    self.dns_resolver = Arc::new(new_resolver);
                }
                Err(e) => {
                    warn!("Failed to reload DNS resolver: {}", e);
                }
            }
        }

        self.config.store(new_config);
        let _ = resp_tx.send(());
    }
}
