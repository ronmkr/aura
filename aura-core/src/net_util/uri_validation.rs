//! URI validation and SSRF mitigation (Decision-0059).
//!
//! Enforces a strict scheme allowlist and blocks private/loopback/link-local
//! destination addresses before any URI enters the download pipeline.
//!
//! Related issues: #241 (RFC1918/link-local SSRF), #244 (file:// exfiltration).

use std::net::IpAddr;

/// Errors produced by [`validate_download_uri`].
#[derive(Debug, PartialEq, Eq)]
pub enum UriValidationError {
    /// URI is empty or cannot be parsed.
    Malformed(String),
    /// Scheme is not in the allowlist (http/https/ftp/ftps/magnet).
    ForbiddenScheme(String),
    /// URI exceeds the 8 192-character safety limit.
    TooLong,
    /// Hostname resolves to a private, loopback, or link-local address.
    PrivateAddress(String),
}

impl std::fmt::Display for UriValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UriValidationError::Malformed(msg) => write!(f, "Malformed URI: {}", msg),
            UriValidationError::ForbiddenScheme(s) => write!(
                f,
                "URI scheme '{}' is not allowed. \
                 Only http, https, ftp, ftps, and magnet URIs are accepted.",
                s
            ),
            UriValidationError::TooLong => {
                write!(f, "URI exceeds maximum length of 8192 characters")
            }
            UriValidationError::PrivateAddress(addr) => write!(
                f,
                "URI resolves to a private or reserved address '{}' which is not permitted",
                addr
            ),
        }
    }
}

const MAX_URI_LEN: usize = 8192;

/// Allowed URI schemes. `magnet:` has no authority component — it is handled
/// specially by the torrent pipeline and carries no network address to validate.
const ALLOWED_SCHEMES: &[&str] = &["http", "https", "ftp", "ftps", "magnet"];

/// Returns `true` if the given [`IpAddr`] falls within a private, loopback,
/// link-local, or otherwise reserved range that must not be contacted by the
/// download engine.
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()           // 127.0.0.0/8
                || v4.is_private()     // 10/8, 172.16/12, 192.168/16
                || v4.is_link_local()  // 169.254.0.0/16
                || v4.is_unspecified() // 0.0.0.0
                || v4.is_broadcast() // 255.255.255.255
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()           // ::1
                || v6.is_unspecified() // ::
                || {
                    // Link-local: fe80::/10
                    let seg = v6.segments();
                    (seg[0] & 0xffc0) == 0xfe80
                }
                || {
                    // IPv4-mapped: ::ffff:0:0/96 — check the embedded v4 part
                    v6.to_ipv4_mapped()
                        .map(|v4| is_private_ip(IpAddr::V4(v4)))
                        .unwrap_or(false)
                }
        }
    }
}

/// Validates a URI before it enters the download pipeline.
///
/// # Checks performed
/// 1. Length ≤ 8 192 characters.
/// 2. Scheme is in `["http", "https", "ftp", "ftps", "magnet"]`.
/// 3. Hostname (if present) does not resolve to a private/loopback/link-local IP.
///    Uses synchronous `std::net::ToSocketAddrs` — acceptable at task-creation
///    time (not on the hot path). Magnet URIs skip the DNS check.
///
/// # Errors
/// Returns [`UriValidationError`] describing the first violation found.
pub fn validate_download_uri(uri: &str) -> Result<(), UriValidationError> {
    // 1. Length check
    if uri.len() > MAX_URI_LEN {
        return Err(UriValidationError::TooLong);
    }

    // 2. Parse scheme — cheap split, no full parse needed yet
    let scheme = uri
        .split_once(':')
        .map(|(s, _)| s.to_ascii_lowercase())
        .ok_or_else(|| UriValidationError::Malformed("No scheme separator found".to_string()))?;

    if !ALLOWED_SCHEMES.contains(&scheme.as_str()) {
        return Err(UriValidationError::ForbiddenScheme(scheme));
    }

    // Magnet URIs have no network address to validate
    if scheme == "magnet" {
        return Ok(());
    }

    // 3. Extract host for IP validation
    let parsed = url::Url::parse(uri).map_err(|e| UriValidationError::Malformed(e.to_string()))?;

    let host = match parsed.host_str() {
        Some(h) if !h.is_empty() => h.to_string(),
        _ => return Ok(()), // no host — let the worker fail naturally
    };

    let port = parsed.port_or_known_default().unwrap_or(80);

    // Try to resolve. If DNS fails we let the worker handle it — we only block
    // known-private resolutions.
    if let Ok(addrs) = std::net::ToSocketAddrs::to_socket_addrs(&(host.as_str(), port)) {
        for addr in addrs {
            if is_private_ip(addr.ip()) {
                return Err(UriValidationError::PrivateAddress(addr.ip().to_string()));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "uri_validation_tests.rs"]
mod tests;
