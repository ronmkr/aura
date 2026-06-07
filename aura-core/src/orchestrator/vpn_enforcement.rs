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
        self.handle().resolve_local_addr()
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

impl super::state::OrchestratorHandle {
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
}

#[cfg(test)]
#[path = "vpn_enforcement_tests.rs"]
mod tests;
