use super::{VpnProvider, VpnStatus};
use crate::{Error, Result};
use async_trait::async_trait;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

/// WireGuard controller using `wg` and `wg-quick` CLI.
pub struct WireGuardProvider {
    interface: String,
    timeout_secs: u64,
}

impl WireGuardProvider {
    pub fn new(interface: String, timeout_secs: u64) -> Self {
        Self {
            interface,
            timeout_secs,
        }
    }

    async fn run_cmd(&self, program: &str, args: &[&str]) -> Result<std::process::Output> {
        let mut cmd = Command::new(program);
        for arg in args {
            cmd.arg(arg);
        }

        let child_fut = cmd.output();
        match timeout(Duration::from_secs(self.timeout_secs), child_fut).await {
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
