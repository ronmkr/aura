//! net_util: Utilities for interface binding and low-level socket control.

use crate::{Error, Result};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::{TcpStream, UdpSocket};

/// Binds a socket to a specific network interface or local IP.
pub fn bind_socket(
    socket: &Socket,
    interface: Option<&str>,
    local_addr: Option<IpAddr>,
) -> Result<()> {
    if let Some(addr) = local_addr {
        let sock_addr = SocketAddr::new(addr, 0).into();
        socket
            .bind(&sock_addr)
            .map_err(|e| Error::Config(format!("Failed to bind to local IP {}: {}", addr, e)))?;
    }

    #[cfg(target_os = "linux")]
    if let Some(iface) = interface {
        socket
            .bind_device(Some(iface.as_bytes()))
            .map_err(|e| Error::Config(format!("Failed to bind to interface {}: {}", iface, e)))?;
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    if let Some(_iface) = interface {
        // macOS uses a different approach for interface binding
        // For now, we might need to resolve the interface to an IP
        // or use IP_BOUND_IF if we were using raw setsockopt.
        // socket2 doesn't have a direct cross-platform bind_device.
        // A common way on macOS is to bind to the IP assigned to that interface.
    }

    Ok(())
}

/// Creates a bound TCP stream, optionally routed through a SOCKS5 proxy.
pub async fn connect_tcp_bound(
    remote_addr: SocketAddr,
    interface: Option<&str>,
    local_addr: Option<IpAddr>,
    proxy: Option<&str>,
) -> Result<TcpStream> {
    let target_for_socket = if let Some(p) = proxy {
        if let Some(proxy_addr) = p.strip_prefix("socks5://") {
            // Resolve proxy address to determine socket domain
            tokio::net::lookup_host(proxy_addr)
                .await
                .map_err(|e| {
                    Error::Config(format!(
                        "Failed to resolve proxy address {}: {}",
                        proxy_addr, e
                    ))
                })?
                .next()
                .ok_or_else(|| {
                    Error::Config(format!("Could not resolve proxy address: {}", proxy_addr))
                })?
        } else {
            return Err(Error::Config(format!(
                "Unsupported proxy scheme for TCP: {}",
                p
            )));
        }
    } else {
        remote_addr
    };

    let domain = if target_for_socket.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };

    let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))
        .map_err(|e| Error::Config(format!("Failed to create TCP socket: {}", e)))?;

    bind_socket(&socket, interface, local_addr)?;

    // Set non-blocking for tokio
    socket
        .set_nonblocking(true)
        .map_err(|e| Error::Config(format!("Failed to set non-blocking: {}", e)))?;

    let stream = TcpStream::connect(target_for_socket).await.map_err(|e| {
        Error::Protocol(format!("Failed to connect to {}: {}", target_for_socket, e))
    })?;

    if let Some(p) = proxy {
        if p.starts_with("socks5://") {
            tracing::debug!("Negotiating SOCKS5 proxy connection to {}", remote_addr);
            let socks_stream =
                tokio_socks::tcp::Socks5Stream::connect_with_socket(stream, remote_addr)
                    .await
                    .map_err(|e| Error::Protocol(format!("SOCKS5 negotiation failed: {}", e)))?;
            return Ok(socks_stream.into_inner());
        }
    }

    Ok(stream)
}

/// Creates a bound UDP socket.
pub async fn bind_udp_bound(
    local_port: u16,
    interface: Option<&str>,
    local_addr: Option<IpAddr>,
) -> Result<UdpSocket> {
    let bind_addr = SocketAddr::new(
        local_addr.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
        local_port,
    );
    let domain = if bind_addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };

    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))
        .map_err(|e| Error::Config(format!("Failed to create UDP socket: {}", e)))?;

    bind_socket(&socket, interface, local_addr)?;

    // Set non-blocking for tokio
    socket
        .set_nonblocking(true)
        .map_err(|e| Error::Config(format!("Failed to set non-blocking: {}", e)))?;

    let std_socket: std::net::UdpSocket = socket.into();
    let udp = UdpSocket::from_std(std_socket)
        .map_err(|e| Error::Config(format!("Failed to convert to tokio UDP socket: {}", e)))?;

    Ok(udp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, SocketAddrV4};

    #[tokio::test]
    async fn test_unsupported_proxy_scheme() {
        let remote_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 80));
        let result = connect_tcp_bound(
            remote_addr,
            None,
            None,
            Some("http://proxy.example.com:8080"),
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            Error::Config(msg) => {
                assert!(msg.contains("Unsupported proxy scheme for TCP"));
            }
            _ => panic!("Expected Error::Config"),
        }
    }
}

