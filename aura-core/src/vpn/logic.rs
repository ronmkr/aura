use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::timeout;

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

    async fn run_cmd(&self, program: &str, args: &[&str]) -> Result<std::process::Output> {
        let mut cmd = Command::new(program);
        for arg in args {
            cmd.arg(arg);
        }

        let child_fut = cmd.output();
        match timeout(Duration::from_secs(5), child_fut).await {
            Ok(Ok(output)) => Ok(output),
            Ok(Err(e)) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Err(Error::Engine(format!(
                        "VPN CLI utility '{}' is not installed or not in the system PATH.",
                        program
                    )))
                } else {
                    Err(Error::Engine(format!(
                        "Failed to execute VPN CLI '{}': {}",
                        program, e
                    )))
                }
            }
            Err(_) => Err(Error::Engine(format!(
                "VPN CLI '{}' execution timed out",
                program
            ))),
        }
    }
}

#[async_trait]
impl VpnProvider for WireGuardProvider {
    fn name(&self) -> &str {
        "wireguard"
    }

    async fn connect(&self) -> Result<()> {
        let output = self.run_cmd("wg-quick", &["up", &self.interface]).await?;

        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Engine(format!(
                "wg-quick up failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )))
        }
    }

    async fn disconnect(&self) -> Result<()> {
        let output = self.run_cmd("wg-quick", &["down", &self.interface]).await?;

        if output.status.success() {
            Ok(())
        } else {
            Err(Error::Engine(format!(
                "wg-quick down failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )))
        }
    }

    async fn status(&self) -> Result<VpnStatus> {
        match self.run_cmd("wg", &["show", &self.interface]).await {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.contains("latest handshake") {
                    Ok(VpnStatus::Connected)
                } else {
                    Ok(VpnStatus::Connecting)
                }
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("does not exist") {
                    Ok(VpnStatus::Disconnected)
                } else {
                    Ok(VpnStatus::Error(stderr.trim().to_string()))
                }
            }
            Err(_) => Ok(VpnStatus::Disconnected),
        }
    }

    fn interface(&self) -> Option<String> {
        Some(self.interface.clone())
    }
}

/// OpenVPN controller using the Management Interface.
pub struct OpenVpnProvider {
    mgmt_addr: String,
    password: Option<String>,
}

impl OpenVpnProvider {
    pub fn new(mgmt_addr: String) -> Self {
        Self {
            mgmt_addr,
            password: None,
        }
    }

    pub fn with_password(mut self, password: String) -> Self {
        self.password = Some(password);
        self
    }

    async fn read_until_prompt(&self, reader: &mut BufReader<TcpStream>) -> Result<()> {
        let mut line = String::new();
        loop {
            let mut buf = [0u8; 256];
            let n = reader.read(&mut buf).await.map_err(|e| {
                Error::Engine(format!("Failed to read from OpenVPN management: {}", e))
            })?;
            if n == 0 {
                return Err(Error::Engine(
                    "OpenVPN management connection closed by peer".to_string(),
                ));
            }
            let chunk = String::from_utf8_lossy(&buf[..n]);
            line.push_str(&chunk);

            if line.contains("ENTER PASSWORD:") {
                if let Some(ref pwd) = self.password {
                    let stream = reader.get_mut();
                    stream
                        .write_all(format!("{}\n", pwd).as_bytes())
                        .await
                        .map_err(|e| {
                            Error::Engine(format!(
                                "Failed to send password to OpenVPN management: {}",
                                e
                            ))
                        })?;
                    line.clear();
                } else {
                    return Err(Error::Engine(
                        "OpenVPN management requires authentication password".to_string(),
                    ));
                }
            }

            if line.contains(">INFO:") || line.contains("SUCCESS: password is correct") {
                break;
            }
        }
        Ok(())
    }

    async fn send_command(&self, cmd: &str) -> Result<String> {
        let connect_fut = TcpStream::connect(&self.mgmt_addr);
        let stream = match timeout(Duration::from_secs(5), connect_fut).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                return Err(Error::Engine(format!(
                    "Failed to connect to OpenVPN management at {}: {}",
                    self.mgmt_addr, e
                )))
            }
            Err(_) => {
                return Err(Error::Engine(
                    "Connection to OpenVPN management timed out".to_string(),
                ))
            }
        };

        let mut reader = BufReader::new(stream);

        // Greet & Handshake
        let handshake_fut = self.read_until_prompt(&mut reader);
        timeout(Duration::from_secs(5), handshake_fut)
            .await
            .map_err(|_| Error::Engine("OpenVPN management handshake timed out".to_string()))??;

        // Send Command
        let stream = reader.get_mut();
        stream
            .write_all(format!("{}\n", cmd).as_bytes())
            .await
            .map_err(|e| {
                Error::Engine(format!("Failed to send OpenVPN management command: {}", e))
            })?;

        // Read Response
        let mut response = String::new();
        let mut line = String::new();
        loop {
            let read_line_fut = reader.read_line(&mut line);
            let n = match timeout(Duration::from_secs(5), read_line_fut).await {
                Ok(Ok(n)) => n,
                Ok(Err(e)) => {
                    return Err(Error::Engine(format!(
                        "Failed to read OpenVPN response line: {}",
                        e
                    )))
                }
                Err(_) => {
                    return Err(Error::Engine(
                        "OpenVPN command response timed out".to_string(),
                    ))
                }
            };

            if n == 0 {
                break;
            }
            response.push_str(&line);
            if line.contains("END") || line.contains("SUCCESS:") || line.contains("ERROR:") {
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
        let res = self.send_command("signal SIGUSR1").await?;
        if res.contains("SUCCESS:") || res.contains("SUCCESS") {
            Ok(())
        } else {
            Err(Error::Engine(format!(
                "OpenVPN reconnect signal failed: {}",
                res.trim()
            )))
        }
    }

    async fn disconnect(&self) -> Result<()> {
        let res = self.send_command("signal SIGTERM").await?;
        if res.contains("SUCCESS:") || res.contains("SUCCESS") {
            Ok(())
        } else {
            Err(Error::Engine(format!(
                "OpenVPN disconnect signal failed: {}",
                res.trim()
            )))
        }
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
        None
    }
}
