use super::{VpnProvider, VpnStatus};
use crate::Result;
use async_trait::async_trait;

/// A simple implementation that just checks if a specific interface is up.
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
