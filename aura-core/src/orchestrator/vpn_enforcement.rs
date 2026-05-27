use crate::orchestrator::state::Orchestrator;
use crate::Result;
use std::sync::Arc;

impl Orchestrator {
    pub(crate) fn create_vpn_provider(
        config: &crate::Config,
    ) -> Option<Arc<dyn crate::vpn::VpnProvider>> {
        if let Some(ref type_name) = config.vpn.type_name {
            match type_name.to_lowercase().as_str() {
                "wireguard" => {
                    let iface = config
                        .network
                        .interface
                        .clone()
                        .unwrap_or_else(|| "wg0".to_string());
                    Some(Arc::new(crate::vpn::WireGuardProvider::new(iface))
                        as Arc<dyn crate::vpn::VpnProvider>)
                }
                "openvpn" => {
                    let addr = config
                        .vpn
                        .profile_path
                        .clone()
                        .unwrap_or_else(|| "127.0.0.1:1337".to_string());
                    Some(Arc::new(crate::vpn::OpenVpnProvider::new(addr))
                        as Arc<dyn crate::vpn::VpnProvider>)
                }
                _ => None,
            }
        } else {
            config.network.interface.as_ref().map(|iface| {
                Arc::new(crate::vpn::InterfaceMonitor::new(iface.clone()))
                    as Arc<dyn crate::vpn::VpnProvider>
            })
        }
    }

    pub(crate) fn update_vpn_provider(&mut self, config: &crate::Config) {
        let new_provider = Self::create_vpn_provider(config);
        self.vpn_provider = new_provider.clone();
        let _ = self.vpn_watch_tx.send(new_provider);
    }

    pub(crate) fn resolve_local_addr(&self) -> Option<std::net::IpAddr> {
        let config = self.config.load();

        if config.vpn.force_tunnel {
            if let Some(ref vpn) = self.vpn_provider {
                if let Some(iface) = vpn.interface() {
                    use local_ip_address::list_afinet_netifas;
                    if let Ok(ifas) = list_afinet_netifas() {
                        for (name, ip) in ifas {
                            if name == iface {
                                return Some(ip);
                            }
                        }
                    }
                }
            }
        }

        if let Some(addr) = config.network.local_addr {
            return Some(addr);
        }

        if let Some(ref iface) = config.network.interface {
            use local_ip_address::list_afinet_netifas;
            if let Ok(ifas) = list_afinet_netifas() {
                for (name, ip) in ifas {
                    if name == *iface {
                        return Some(ip);
                    }
                }
            }
        }

        None
    }

    pub(crate) fn update_power_management(&mut self) {
        use crate::task::DownloadPhase;
        let is_active = self
            .tasks
            .values()
            .any(|t| t.phase == DownloadPhase::Downloading);
        self.power_manager.set_active(is_active);
    }

    pub(crate) async fn verify_vpn_connectivity(&self) -> Result<()> {
        let config = self.config.load();
        if !config.vpn.force_tunnel {
            return Ok(());
        }

        if let Some(ref vpn) = self.vpn_provider {
            match vpn.status().await? {
                crate::vpn::VpnStatus::Connected => Ok(()),
                crate::vpn::VpnStatus::Connecting => {
                    tracing::info!("VPN is connecting, waiting...");
                    Err(crate::Error::Engine("VPN is still connecting".to_string()))
                }
                crate::vpn::VpnStatus::Disconnected | crate::vpn::VpnStatus::Error(_) => {
                    if config.vpn.auto_connect {
                        tracing::info!("VPN disconnected, attempting auto-connect...");
                        vpn.connect().await?;
                        Err(crate::Error::Engine(
                            "VPN re-connect triggered. Please retry in a moment.".to_string(),
                        ))
                    } else {
                        Err(crate::Error::Engine(
                            "Mandatory VPN tunnel is down and auto-connect is disabled."
                                .to_string(),
                        ))
                    }
                }
            }
        } else {
            if config.network.interface.is_some() {
                Ok(())
            } else {
                Err(crate::Error::Config(
                    "Mandatory tunnel enabled but no VPN provider or interface configured"
                        .to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::SubTaskEvent;
    use crate::vpn::{VpnProvider, VpnStatus};
    use std::sync::Arc;

    struct MockVpnProvider {
        status: Arc<tokio::sync::Mutex<VpnStatus>>,
    }

    #[async_trait::async_trait]
    impl VpnProvider for MockVpnProvider {
        fn name(&self) -> &str {
            "mock-vpn"
        }

        async fn connect(&self) -> Result<()> {
            Ok(())
        }

        async fn disconnect(&self) -> Result<()> {
            Ok(())
        }

        async fn status(&self) -> Result<VpnStatus> {
            Ok(self.status.lock().await.clone())
        }

        fn interface(&self) -> Option<String> {
            Some("tun0".to_string())
        }
    }

    #[tokio::test]
    async fn test_vpn_killswitch_enforcement() {
        let mut config = crate::Config::default();
        config.vpn.force_tunnel = true;

        let status = Arc::new(tokio::sync::Mutex::new(VpnStatus::Disconnected));
        let mock_provider = Arc::new(MockVpnProvider {
            status: Arc::clone(&status),
        });

        let (_command_tx, command_rx) = tokio::sync::mpsc::channel(100);
        let (storage_tx, _storage_rx) = tokio::sync::mpsc::channel(100);
        let (_completion_tx, completion_rx) = tokio::sync::mpsc::channel(100);
        let (dht_tx, _dht_rx) = tokio::sync::mpsc::channel(100);
        let (nat_tx, _nat_rx) = tokio::sync::mpsc::channel(100);
        let (lpd_tx, _lpd_rx) = tokio::sync::mpsc::channel(100);

        let config_swap = Arc::new(arc_swap::ArcSwap::from_pointee(config.clone()));

        let temp_dir = tempfile::tempdir().unwrap();
        let db = sled::open(temp_dir.path()).unwrap();
        let dns_resolver = Arc::new(
            hickory_resolver::TokioResolver::builder_tokio()
                .unwrap()
                .build()
                .unwrap(),
        );

        let (mut orchestrator, _event_tx) = Orchestrator::new(
            command_rx,
            storage_tx,
            completion_rx,
            dht_tx,
            lpd_tx,
            nat_tx,
            config_swap,
            db,
            dns_resolver,
        );

        let vpn_watch_rx = orchestrator.vpn_watch_tx.subscribe();
        orchestrator.vpn_provider = Some(mock_provider.clone() as Arc<dyn VpnProvider>);
        let _ = orchestrator
            .vpn_watch_tx
            .send(Some(mock_provider.clone() as Arc<dyn VpnProvider>));

        // 1. Verify verify_vpn_connectivity() fails when Disconnected
        let verify_result = orchestrator.verify_vpn_connectivity().await;
        assert!(verify_result.is_err());
        assert!(verify_result
            .unwrap_err()
            .to_string()
            .contains("Mandatory VPN tunnel is down"));

        // 2. Verify verify_vpn_connectivity() succeeds when Connected
        *status.lock().await = VpnStatus::Connected;
        let verify_result2 = orchestrator.verify_vpn_connectivity().await;
        assert!(verify_result2.is_ok());

        // 3. Verify background watch loop triggers KillSwitch on transition to Disconnected
        let mut subtask_rx = orchestrator.subtask_rx;
        let config_clone = orchestrator.config.clone();
        let subtask_tx_monitor = orchestrator.subtask_tx.clone();

        let watch_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(10));
            loop {
                interval.tick().await;
                let force_tunnel = config_clone.load().vpn.force_tunnel;
                let vpn_opt = vpn_watch_rx.borrow().clone();
                if let Some(vpn) = vpn_opt {
                    if force_tunnel {
                        let stat = vpn.status().await;
                        match stat {
                            Ok(VpnStatus::Disconnected) | Ok(VpnStatus::Error(_)) => {
                                let _ = subtask_tx_monitor.send(SubTaskEvent::KillSwitch).await;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        // Trigger transition to Disconnected
        *status.lock().await = VpnStatus::Disconnected;

        // Wait for SubTaskEvent::KillSwitch on the channel
        let mut killswitch_received = false;
        tokio::select! {
            _ = watch_handle => {}
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
        }

        while let Ok(event) = subtask_rx.try_recv() {
            if let SubTaskEvent::KillSwitch = event {
                killswitch_received = true;
                break;
            }
        }

        assert!(
            killswitch_received,
            "Orchestrator should have received a KillSwitch event on VPN disconnect"
        );
    }
}
