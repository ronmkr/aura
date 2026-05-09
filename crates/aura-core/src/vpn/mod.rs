use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VpnStatus {
    Connected,
    Disconnected,
    Connecting,
    Error(String),
}

/// The core trait for VPN provider implementations (OpenVPN, WireGuard, etc.).
#[async_trait]
pub trait VpnProvider: Send + Sync {
    /// Returns the name of the VPN provider.
    fn name(&self) -> &str;

    /// Attempts to connect to the VPN.
    async fn connect(&self) -> Result<()>;

    /// Disconnects from the VPN.
    async fn disconnect(&self) -> Result<()>;

    /// Returns the current status of the VPN connection.
    async fn status(&self) -> Result<VpnStatus>;

    /// Returns the network interface name used by this VPN.
    fn interface(&self) -> Option<String>;
}

/// A simple implementation that just checks if a specific interface is up.
/// Acts as a bridge for the current "Kill-switch" logic.
pub struct InterfaceMonitor {
    interface_name: String,
}

impl InterfaceMonitor {
    pub fn new(interface_name: String) -> Self {
        Self { interface_name }
    }
}

#[async_trait]
impl VpnProvider for InterfaceMonitor {
    fn name(&self) -> &str {
        "interface-monitor"
    }

    async fn connect(&self) -> Result<()> {
        // This provider doesn't support active connection control
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus> {
        let iface = self.interface_name.clone();
        let is_up = tokio::task::spawn_blocking(move || {
            use local_ip_address::list_afinet_netifas;
            list_afinet_netifas()
                .ok()
                .map(|ifas| ifas.into_iter().any(|(name, _)| name == iface))
        })
        .await
        .unwrap_or(None)
        .unwrap_or(false);

        if is_up {
            Ok(VpnStatus::Connected)
        } else {
            Ok(VpnStatus::Disconnected)
        }
    }

    fn interface(&self) -> Option<String> {
        Some(self.interface_name.clone())
    }
}
