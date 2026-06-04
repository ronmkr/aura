use super::{is_private_ip, validate_download_uri, UriValidationError};
use std::net::IpAddr;

// --- URI Scheme Validation ---

#[test]
fn test_allows_https() {
    assert!(validate_download_uri("https://example.com/file.zip").is_ok());
}

#[test]
fn test_allows_ftp() {
    assert!(validate_download_uri("ftp://ftp.example.com/pub/file.tar.gz").is_ok());
}

#[test]
fn test_allows_magnet() {
    assert!(validate_download_uri("magnet:?xt=urn:btih:abc123&dn=test").is_ok());
}

#[test]
fn test_rejects_file_scheme() {
    let err = validate_download_uri("file:///etc/passwd").unwrap_err();
    assert!(matches!(err, UriValidationError::ForbiddenScheme(_)));
}

#[test]
fn test_rejects_file_shadow() {
    let err = validate_download_uri("file:///etc/shadow").unwrap_err();
    assert!(matches!(err, UriValidationError::ForbiddenScheme(_)));
}

#[test]
fn test_rejects_data_scheme() {
    let err = validate_download_uri("data:text/html,<h1>hi</h1>").unwrap_err();
    assert!(matches!(err, UriValidationError::ForbiddenScheme(_)));
}

#[test]
fn test_rejects_javascript_scheme() {
    let err = validate_download_uri("javascript:alert(1)").unwrap_err();
    assert!(matches!(err, UriValidationError::ForbiddenScheme(_)));
}

#[test]
fn test_rejects_blob_scheme() {
    let err = validate_download_uri("blob:https://example.com/uuid").unwrap_err();
    assert!(matches!(err, UriValidationError::ForbiddenScheme(_)));
}

#[test]
fn test_rejects_too_long_uri() {
    let long_uri = format!("https://example.com/{}", "a".repeat(8200));
    assert!(matches!(
        validate_download_uri(&long_uri).unwrap_err(),
        UriValidationError::TooLong
    ));
}

#[test]
fn test_rejects_no_scheme() {
    assert!(matches!(
        validate_download_uri("example.com/file").unwrap_err(),
        UriValidationError::Malformed(_)
    ));
}

// --- IP Classification ---

#[test]
fn test_is_private_loopback_v4() {
    use std::net::Ipv4Addr;
    assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
}

#[test]
fn test_is_private_rfc1918() {
    use std::net::Ipv4Addr;
    assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
    assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
    assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
    assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(172, 31, 255, 255))));
}

#[test]
fn test_is_private_link_local() {
    use std::net::Ipv4Addr;
    assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 0, 1))));
    assert!(is_private_ip(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
}

#[test]
fn test_is_private_v6_loopback() {
    assert!(is_private_ip("::1".parse().unwrap()));
}

#[test]
fn test_is_private_v6_link_local() {
    assert!(is_private_ip("fe80::1".parse().unwrap()));
}

#[test]
fn test_is_not_private_public_v4() {
    use std::net::Ipv4Addr;
    assert!(!is_private_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
    assert!(!is_private_ip(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
}

#[test]
fn test_is_not_private_public_v6() {
    assert!(!is_private_ip("2001:4860:4860::8888".parse().unwrap()));
}
