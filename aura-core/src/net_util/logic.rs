//! net_util: Utilities for interface binding and low-level socket control.

use crate::{Error, Result};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::{TcpStream, UdpSocket};

/// Attempts to negotiate Kernel TLS (kTLS) on a TCP socket for zero-copy performance.
/// Acting as a no-op on non-supported platforms (macOS/Windows).
#[cfg(unix)]
pub fn try_enable_ktls<S: std::os::unix::io::AsRawFd>(_stream: &S) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let fd = _stream.as_raw_fd();
        let tls = b"tls\0";
        let ret = unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_TCP,
                libc::TCP_ULP,
                tls.as_ptr() as *const libc::c_void,
                tls.len() as libc::socklen_t,
            )
        };
        if ret < 0 {
            let err = std::io::Error::last_os_error();
            tracing::warn!("Failed to negotiate kTLS (TCP_ULP): {}", err);
            return Err(err);
        }
        tracing::info!("Successfully negotiated kTLS (TCP_ULP) on socket");
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn try_enable_ktls<S>(_stream: &S) -> std::io::Result<()> {
    Ok(())
}

/// Binds a socket to a specific network interface or local IP.
pub fn bind_socket(
    socket: &Socket,
    _interface: Option<&str>,
    local_addr: Option<IpAddr>,
) -> Result<()> {
    if let Some(addr) = local_addr {
        let sock_addr = SocketAddr::new(addr, 0).into();
        socket
            .bind(&sock_addr)
            .map_err(|e| Error::Config(format!("Failed to bind to local IP {}: {}", addr, e)))?;
    }

    #[cfg(target_os = "linux")]
    if let Some(iface) = _interface {
        socket
            .bind_device(Some(iface.as_bytes()))
            .map_err(|e| Error::Config(format!("Failed to bind to interface {}: {}", iface, e)))?;
    }

    Ok(())
}

async fn race_connect(
    addrs: Vec<SocketAddr>,
    _interface: Option<&str>,
    local_addr: Option<IpAddr>,
) -> Result<TcpStream> {
    if addrs.is_empty() {
        return Err(Error::Config(
            "No addresses provided for racing".to_string(),
        ));
    }

    if addrs.len() == 1 {
        let addr = addrs[0];
        let socket = if addr.is_ipv4() {
            tokio::net::TcpSocket::new_v4()
        } else {
            tokio::net::TcpSocket::new_v6()
        }
        .map_err(|e| Error::Config(format!("Failed to create TCP socket: {}", e)))?;

        if let Some(l_addr) = local_addr {
            socket.bind(SocketAddr::new(l_addr, 0)).map_err(|e| {
                Error::Config(format!("Failed to bind to local IP {}: {}", l_addr, e))
            })?;
        }

        let stream = socket
            .connect(addr)
            .await
            .map_err(|e| Error::Protocol(format!("Failed to connect to {}: {}", addr, e)))?;

        let _ = try_enable_ktls(&stream);
        return Ok(stream);
    }

    // Race addresses with a 250ms staggered start
    let mut futures = futures_util::stream::FuturesUnordered::new();

    for (i, addr) in addrs.into_iter().enumerate() {
        let l_addr = local_addr;

        futures.push(async move {
            if i > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(i as u64 * 250)).await;
            }

            let socket = if addr.is_ipv4() {
                tokio::net::TcpSocket::new_v4().ok()?
            } else {
                tokio::net::TcpSocket::new_v6().ok()?
            };

            if let Some(la) = l_addr {
                let _ = socket.bind(SocketAddr::new(la, 0));
            }

            socket.connect(addr).await.ok().map(|s| {
                let _ = try_enable_ktls(&s);
                (s, addr)
            })
        });
    }

    use futures_util::StreamExt;
    while let Some(res) = futures.next().await {
        if let Some((stream, addr)) = res {
            tracing::debug!("Connected to {} (winner of racing)", addr);
            return Ok(stream);
        }
    }

    Err(Error::Protocol(
        "All connection attempts failed during racing".to_string(),
    ))
}

/// Creates a bound TCP stream, optionally routed through a SOCKS5 proxy.
/// Implements Happy Eyeballs (RFC 8305) style racing for dual-stack connectivity.
pub async fn connect_tcp_bound(
    remote_addr: SocketAddr,
    interface: Option<&str>,
    local_addr: Option<IpAddr>,
    proxy: Option<&str>,
) -> Result<TcpStream> {
    if let Some(p) = proxy {
        if let Some(proxy_addr) = p.strip_prefix("socks5://") {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host(proxy_addr)
                .await
                .map_err(|e| {
                    Error::Config(format!(
                        "Failed to resolve proxy address {}: {}",
                        proxy_addr, e
                    ))
                })?
                .collect();

            let stream = race_connect(addrs, interface, local_addr).await?;

            tracing::debug!("Negotiating SOCKS5 proxy connection to {}", remote_addr);
            let socks_stream =
                tokio_socks::tcp::Socks5Stream::connect_with_socket(stream, remote_addr)
                    .await
                    .map_err(|e| Error::Protocol(format!("SOCKS5 negotiation failed: {}", e)))?;
            return Ok(socks_stream.into_inner());
        } else {
            return Err(Error::Config(format!(
                "Unsupported proxy scheme for TCP: {}",
                p
            )));
        }
    }

    // Single remote_addr provided. Usually workers have already resolved.
    // In a future PR, we should accept (host, port) to perform DNS racing here.
    race_connect(vec![remote_addr], interface, local_addr).await
}

/// Creates a bound TCP stream to a host/port or SocketAddr, optionally routed through a SOCKS5 proxy.
/// Implements dual-stack DNS resolution and Happy Eyeballs (RFC 8305) style racing.
pub async fn connect_tcp_bound_host(
    host: &str,
    port: u16,
    interface: Option<&str>,
    local_addr: Option<IpAddr>,
    proxy: Option<&str>,
) -> Result<TcpStream> {
    let mut resolved_addrs = Vec::new();
    let target = format!("{}:{}", host, port);
    match tokio::net::lookup_host(&target).await {
        Ok(addrs) => {
            let mut ipv6_addrs = Vec::new();
            let mut ipv4_addrs = Vec::new();
            for addr in addrs {
                if addr.is_ipv6() {
                    ipv6_addrs.push(addr);
                } else {
                    ipv4_addrs.push(addr);
                }
            }

            let mut i = 0;
            let mut j = 0;
            while i < ipv6_addrs.len() || j < ipv4_addrs.len() {
                if i < ipv6_addrs.len() {
                    resolved_addrs.push(ipv6_addrs[i]);
                    i += 1;
                }
                if j < ipv4_addrs.len() {
                    resolved_addrs.push(ipv4_addrs[j]);
                    j += 1;
                }
            }
        }
        Err(e) => {
            return Err(Error::Config(format!(
                "Failed to resolve host {}: {}",
                host, e
            )));
        }
    }

    if resolved_addrs.is_empty() {
        return Err(Error::Config(format!(
            "No addresses resolved for host {}",
            host
        )));
    }

    if let Some(p) = proxy {
        if let Some(proxy_addr) = p.strip_prefix("socks5://") {
            let proxy_resolved: Vec<SocketAddr> = tokio::net::lookup_host(proxy_addr)
                .await
                .map_err(|e| {
                    Error::Config(format!(
                        "Failed to resolve proxy address {}: {}",
                        proxy_addr, e
                    ))
                })?
                .collect();

            let stream = race_connect(proxy_resolved, interface, local_addr).await?;

            tracing::debug!(
                "Negotiating SOCKS5 proxy connection to resolved target {:?}",
                resolved_addrs[0]
            );
            let socks_stream =
                tokio_socks::tcp::Socks5Stream::connect_with_socket(stream, resolved_addrs[0])
                    .await
                    .map_err(|e| Error::Protocol(format!("SOCKS5 negotiation failed: {}", e)))?;
            return Ok(socks_stream.into_inner());
        } else {
            return Err(Error::Config(format!(
                "Unsupported proxy scheme for TCP: {}",
                p
            )));
        }
    }

    race_connect(resolved_addrs, interface, local_addr).await
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

    socket
        .set_nonblocking(true)
        .map_err(|e| Error::Config(format!("Failed to set non-blocking: {}", e)))?;

    let std_socket: std::net::UdpSocket = socket.into();
    let udp = UdpSocket::from_std(std_socket)
        .map_err(|e| Error::Config(format!("Failed to convert to tokio UDP socket: {}", e)))?;

    Ok(udp)
}

#[cfg(test)]
#[path = "logic_tests.rs"]
mod tests;
