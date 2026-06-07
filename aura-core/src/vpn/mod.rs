use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

pub mod monitor;
pub mod openvpn;
pub mod wireguard;

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

pub use monitor::InterfaceMonitor;
pub use openvpn::OpenVpnProvider;
pub use wireguard::WireGuardProvider;
