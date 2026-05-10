use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::Command;

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

/// WireGuard controller using `wg` and `wg-quick` CLI.
pub struct WireGuardProvider {
    interface: String,
}

impl WireGuardProvider {
    pub fn new(interface: String) -> Self {
        Self { interface }
    }
}

#[async_trait]
impl VpnProvider for WireGuardProvider {
    fn name(&self) -> &str {
        "wireguard"
    }

    async fn connect(&self) -> Result<()> {
        let output = Command::new("wg-quick")
            .arg("up")
            .arg(&self.interface)
            .output()
            .await?;

        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Engine(format!(
                "wg-quick up failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )))
        }
    }

    async fn disconnect(&self) -> Result<()> {
        let _ = Command::new("wg-quick")
            .arg("down")
            .arg(&self.interface)
            .output()
            .await;
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus> {
        let output = Command::new("wg")
            .arg("show")
            .arg(&self.interface)
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if stdout.contains("latest handshake") {
                    Ok(VpnStatus::Connected)
                } else {
                    Ok(VpnStatus::Connecting)
                }
            }
            _ => Ok(VpnStatus::Disconnected),
        }
    }

    fn interface(&self) -> Option<String> {
        Some(self.interface.clone())
    }
}

/// OpenVPN controller using the Management Interface.
pub struct OpenVpnProvider {
    mgmt_addr: String,
}

impl OpenVpnProvider {
    pub fn new(mgmt_addr: String) -> Self {
        Self { mgmt_addr }
    }

    async fn send_command(&self, cmd: &str) -> Result<String> {
        let mut stream = TcpStream::connect(&self.mgmt_addr).await.map_err(|e| {
            Error::Engine(format!(
                "Failed to connect to OpenVPN management at {}: {}",
                self.mgmt_addr, e
            ))
        })?;

        stream.write_all(format!("{}\n", cmd).as_bytes()).await?;

        let mut reader = BufReader::new(stream);
        let mut response = String::new();
        let mut line = String::new();

        // OpenVPN mgmt usually ends multi-line output with "END" or "SUCCESS"
        while reader.read_line(&mut line).await? > 0 {
            response.push_str(&line);
            if line.contains("END") || line.contains("SUCCESS") || line.contains("ERROR") {
                break;
            }
            line.clear();
        }

        Ok(response)
    }
}

#[async_trait]
impl VpnProvider for OpenVpnProvider {
    fn name(&self) -> &str {
        "openvpn"
    }

    async fn connect(&self) -> Result<()> {
        // Trigger reconnection
        let _ = self.send_command("signal SIGUSR1").await?;
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        let _ = self.send_command("signal SIGTERM").await?;
        Ok(())
    }

    async fn status(&self) -> Result<VpnStatus> {
        match self.send_command("state").await {
            Ok(res) => {
                if res.contains("CONNECTED") {
                    Ok(VpnStatus::Connected)
                } else if res.contains("CONNECTING") || res.contains("WAIT") || res.contains("AUTH")
                {
                    Ok(VpnStatus::Connecting)
                } else {
                    Ok(VpnStatus::Disconnected)
                }
            }
            Err(e) => Ok(VpnStatus::Error(e.to_string())),
        }
    }

    fn interface(&self) -> Option<String> {
        // OpenVPN interface is usually tun0/tap0 but varies
        None
    }
}
