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
        250,
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

#[tokio::test]
async fn test_connect_tcp_bound_host_invalid() {
    let result = connect_tcp_bound_host(
        "nonexistent-domain-name-aura.local",
        80,
        None,
        None,
        None,
        250,
    )
    .await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_connect_tcp_bound_host_localhost() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    let result = connect_tcp_bound_host("127.0.0.1", port, None, None, None, 250).await;

    assert!(
        result.is_ok(),
        "Failed to connect to 127.0.0.1: {:?}",
        result.err()
    );
}
