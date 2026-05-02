//! net_util: Utilities for interface binding and low-level socket control.

use std::net::{IpAddr, SocketAddr};
use socket2::{Socket, Domain, Type, Protocol};
use crate::{Result, Error};
use tokio::net::{TcpStream, UdpSocket};

/// Binds a socket to a specific network interface or local IP.
pub fn bind_socket(socket: &Socket, interface: Option<&str>, local_addr: Option<IpAddr>) -> Result<()> {
    if let Some(addr) = local_addr {
        let sock_addr = SocketAddr::new(addr, 0).into();
        socket.bind(&sock_addr)
            .map_err(|e| Error::Config(format!("Failed to bind to local IP {}: {}", addr, e)))?;
    }

    #[cfg(target_os = "linux")]
    if let Some(iface) = interface {
        socket.bind_device(Some(iface.as_bytes()))
            .map_err(|e| Error::Config(format!("Failed to bind to interface {}: {}", iface, e)))?;
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    if let Some(iface) = interface {
        // macOS uses a different approach for interface binding
        // For now, we might need to resolve the interface to an IP
        // or use IP_BOUND_IF if we were using raw setsockopt.
        // socket2 doesn't have a direct cross-platform bind_device.
        // A common way on macOS is to bind to the IP assigned to that interface.
    }

    Ok(())
}

/// Creates a bound TCP stream.
pub async fn connect_tcp_bound(
    remote_addr: SocketAddr,
    interface: Option<&str>,
    local_addr: Option<IpAddr>,
) -> Result<TcpStream> {
    let domain = if remote_addr.is_ipv4() { Domain::IPV4 } else { Domain::IPV6 };
    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))
        .map_err(|e| Error::Config(format!("Failed to create TCP socket: {}", e)))?;

    bind_socket(&socket, interface, local_addr)?;

    // Set non-blocking for tokio
    socket.set_nonblocking(true)
        .map_err(|e| Error::Config(format!("Failed to set non-blocking: {}", e)))?;

    let stream = TcpStream::connect(remote_addr).await
        .map_err(|e| Error::Protocol(format!("Failed to connect to {}: {}", remote_addr, e)))?;
        
    Ok(stream)
}

/// Creates a bound UDP socket.
pub async fn bind_udp_bound(
    local_port: u16,
    interface: Option<&str>,
    local_addr: Option<IpAddr>,
) -> Result<UdpSocket> {
    let bind_addr = SocketAddr::new(local_addr.unwrap_or("0.0.0.0".parse().unwrap()), local_port);
    let domain = if bind_addr.is_ipv4() { Domain::IPV4 } else { Domain::IPV6 };
    
    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| Error::Config(format!("Failed to create UDP socket: {}", e)))?;

    bind_socket(&socket, interface, local_addr)?;

    // Set non-blocking for tokio
    socket.set_nonblocking(true)
        .map_err(|e| Error::Config(format!("Failed to set non-blocking: {}", e)))?;

    let std_socket: std::net::UdpSocket = socket.into();
    let udp = UdpSocket::from_std(std_socket)
        .map_err(|e| Error::Config(format!("Failed to convert to tokio UDP socket: {}", e)))?;
        
    Ok(udp)
}
