//! nat: Automatic port mapping via UPnP and NAT-PMP/PCP.

use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU16;
use tokio::sync::mpsc;
use tracing::{info, warn, debug};
use crate::{Result, Error};
use igd_next::aio::tokio::search_gateway;
use igd_next::SearchOptions;
use local_ip_address::local_ip;
use crab_nat::{InternetProtocol, PortMapping, PortMappingOptions};

#[derive(Debug)]
pub enum NatCommand {
    MapPort {
        port: u16,
        description: String,
    },
}

pub struct NatActor {
    command_rx: mpsc::Receiver<NatCommand>,
}

impl NatActor {
    pub fn new(command_rx: mpsc::Receiver<NatCommand>) -> Self {
        Self { command_rx }
    }

    pub async fn run(mut self) -> Result<()> {
        info!("NAT Traversal Actor started");
        
        while let Some(cmd) = self.command_rx.recv().await {
            match cmd {
                NatCommand::MapPort { port, description } => {
                    self.perform_mapping(port, &description).await;
                }
            }
        }
        
        Ok(())
    }

    async fn perform_mapping(&self, port: u16, description: &str) {
        // Try UPnP first as it's most common
        match self.try_upnp(port, description).await {
            Ok(_) => {
                info!("Mapped port {} via UPnP", port);
                return;
            }
            Err(e) => {
                debug!("UPnP mapping failed: {}. Falling back to NAT-PMP/PCP.", e);
            }
        }

        // Fallback to NAT-PMP / PCP
        match self.try_nat_pmp_pcp(port).await {
            Ok(_) => {
                info!("Mapped port {} via NAT-PMP/PCP", port);
            }
            Err(e) => {
                warn!("All NAT traversal methods failed for port {}: {}", port, e);
            }
        }
    }

    async fn try_upnp(&self, port: u16, description: &str) -> Result<()> {
        let local_ip = local_ip()
            .map_err(|e| Error::Protocol(format!("Failed to get local IP: {}", e)))?;

        let gateway = search_gateway(SearchOptions::default()).await
            .map_err(|e| Error::Protocol(format!("UPnP discovery failed: {}", e)))?;
            
        let local_socket = SocketAddr::new(local_ip, port);
        
        gateway.add_port(igd_next::PortMappingProtocol::TCP, port, local_socket, 3600, description).await
            .map_err(|e| Error::Protocol(format!("UPnP mapping failed: {}", e)))?;
            
        Ok(())
    }

    async fn try_nat_pmp_pcp(&self, port: u16) -> Result<()> {
        let local_ip = local_ip()
            .map_err(|e| Error::Protocol(format!("Failed to get local IP: {}", e)))?;
        
        // Try to get gateway from UPnP discovery if available
        let gateway = if let Ok(gw) = search_gateway(SearchOptions::default()).await {
            gw.addr.ip()
        } else {
             // Fallback to .1 guess
             if let IpAddr::V4(v4) = local_ip {
                let mut octets = v4.octets();
                octets[3] = 1;
                IpAddr::V4(std::net::Ipv4Addr::from(octets))
             } else {
                 return Err(Error::Protocol("NAT-PMP guess only for IPv4".to_string()));
             }
        };

        let _mapping = PortMapping::new(
            gateway,
            local_ip,
            InternetProtocol::Tcp,
            NonZeroU16::new(port).ok_or_else(|| Error::Protocol("Invalid port".to_string()))?,
            PortMappingOptions::default(),
        ).await.map_err(|e| Error::Protocol(format!("NAT-PMP/PCP error: {:?}", e)))?;
        
        Ok(())
    }
}
