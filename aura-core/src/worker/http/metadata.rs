use super::super::Metadata;
use super::HttpWorker;
use crate::{Error, Result};

/// Parses the filename from a `Content-Disposition` header value, following
/// RFC 6266 §4.3 (prefer `filename*` over `filename`) and RFC 5987 encoding.
///
/// Security guarantees:
/// - Strips all path separators (`/`, `\\`, `:`) to prevent path traversal.
/// - Strips null bytes and ASCII control characters (0x00–0x1F, 0x7F).
/// - Truncates to 255 bytes (POSIX NAME_MAX).
/// - Returns `None` if the result is empty after sanitization.
pub(crate) fn parse_content_disposition(header: &str) -> Option<String> {
    // Prefer filename* (RFC 5987 encoded) over plain filename
    let starred = header.split(';').find_map(|part| {
        let part = part.trim();
        // filename*=UTF-8''encoded%20name
        let rest = part.strip_prefix_ci("filename*=")?;
        // Strip optional charset prefix: UTF-8'' or ISO-8859-1''
        let encoded = if let Some(pos) = rest.find("''") {
            &rest[pos + 2..]
        } else {
            rest
        };
        // Percent-decode manually — avoid pulling in an extra crate
        percent_decode(encoded)
    });

    let raw = if let Some(s) = starred {
        s
    } else {
        // Fall back to plain filename=
        header.split(';').find_map(|part| {
            let part = part.trim();
            let rest = part.strip_prefix_ci("filename=")?;
            Some(rest.trim_matches('"').to_string())
        })?
    };

    sanitize_filename(&raw)
}

/// Case-insensitive prefix strip helper (not yet stable in std as a method).
trait StripPrefixCi {
    fn strip_prefix_ci(&self, prefix: &str) -> Option<&str>;
}
impl StripPrefixCi for str {
    fn strip_prefix_ci(&self, prefix: &str) -> Option<&str> {
        if self.len() >= prefix.len() && self[..prefix.len()].eq_ignore_ascii_case(prefix) {
            Some(&self[prefix.len()..])
        } else {
            None
        }
    }
}

/// Percent-decodes a string like `report%20final.pdf` → `report final.pdf`.
fn percent_decode(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = char::from(bytes[i + 1]).to_digit(16)?;
            let lo = char::from(bytes[i + 2]).to_digit(16)?;
            let byte = ((hi << 4) | lo) as u8;
            out.push(byte as char);
            i += 3;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    Some(out)
}

/// Sanitizes a raw filename string:
/// - Removes path separators and null/control bytes
/// - Truncates to 255 bytes
/// - Returns `None` if result is empty
fn sanitize_filename(raw: &str) -> Option<String> {
    let sanitized: String = raw
        .chars()
        .filter(|&c| {
            c != '/' && c != '\\' && c != ':' // path separators
                && c != '\0'                  // null byte
                && !c.is_control() // ASCII control chars
        })
        .collect();

    // Truncate to POSIX NAME_MAX (255 bytes)
    let truncated = if sanitized.len() > 255 {
        // Find a valid UTF-8 boundary at or before byte 255
        let mut boundary = 255;
        while !sanitized.is_char_boundary(boundary) {
            boundary -= 1;
        }
        sanitized[..boundary].to_string()
    } else {
        sanitized
    };

    if truncated.is_empty() {
        None
    } else {
        Some(truncated)
    }
}

#[cfg(test)]
#[path = "content_disposition_tests.rs"]
mod content_disposition_tests;

impl HttpWorker {
    /// Resolves the final direct link and file size in a single pass.
    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        tracing::debug!(uri = %self.options.uri, "Resolving metadata");
        let mut current_uri = self.options.uri.clone();
        let mut referer: Option<String> = None;
        let mut redirect_count = 0;
        let max_redirects = 20;

        loop {
            current_uri = self.upgrade_url(&current_uri).await;
            let mut attempts = 0;
            let max_attempts = self.options.retry_count;

            let response = loop {
                let res = self
                    .send_request(&current_uri, |client, uri| {
                        let mut request = client.get(uri).header("Range", "bytes=0-0");
                        if let Some(ref ref_uri) = referer {
                            request = request.header("Referer", ref_uri);
                        }

                        if let Some(ref provider) = self.options.credential_provider {
                            if let Ok(url) = url::Url::parse(uri) {
                                if let Some(host) = url.host_str() {
                                    if let Some(creds) = provider.get_credentials(host) {
                                        if let (Some(user), Some(pass)) =
                                            (&creds.login, &creds.password)
                                        {
                                            request = request.basic_auth(user, Some(pass));
                                        }
                                    }
                                }
                            }
                        }
                        request
                    })
                    .await;

                match res {
                    Ok(resp) => {
                        if resp.status().is_success() || resp.status().is_redirection() {
                            break resp;
                        } else if Self::is_retryable(resp.status()) && attempts < max_attempts {
                            attempts += 1;
                            let delay = self.options.retry_delay_secs * (2u64.pow(attempts - 1));
                            tracing::warn!(
                                status = %resp.status(),
                                attempt = attempts,
                                delay_secs = delay,
                                "Transient HTTP error during metadata resolution, retrying"
                            );
                            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                            continue;
                        } else {
                            return Err(Error::Protocol(format!(
                                "Metadata resolution failed with status: {}",
                                resp.status()
                            )));
                        }
                    }
                    Err(e) if attempts < max_attempts => {
                        attempts += 1;
                        let delay = self.options.retry_delay_secs * (2u64.pow(attempts - 1));
                        tracing::warn!(
                            error = %e,
                            attempt = attempts,
                            delay_secs = delay,
                            "HTTP request failed during metadata resolution, retrying"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                        continue;
                    }
                    Err(e) => {
                        return Err(Error::Protocol(format!(
                            "Metadata resolution failed: {}",
                            e
                        )))
                    }
                }
            };

            if response.status().is_redirection() {
                if redirect_count >= max_redirects {
                    return Err(Error::Protocol("Too many redirects".to_string()));
                }
                redirect_count += 1;

                let next_url = response
                    .headers()
                    .get("Location")
                    .and_then(|h| h.to_str().ok())
                    .ok_or_else(|| {
                        Error::Protocol("Redirect without Location header".to_string())
                    })?;

                let resolved_next = url::Url::parse(&current_uri)
                    .and_then(|base| base.join(next_url))
                    .map_err(|e| Error::Protocol(format!("Invalid redirect URL: {}", e)))?
                    .to_string();

                referer = Some(current_uri);
                current_uri = resolved_next;
                continue;
            }

            let is_html = {
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|h| h.to_str().ok());
                content_type
                    .map(|ct| ct.contains("text/html"))
                    .unwrap_or(false)
            };

            if is_html {
                let ct = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|h| h.to_str().ok())
                    .unwrap_or("text/html")
                    .to_string();

                let body = response
                    .text()
                    .await
                    .map_err(|e| Error::Protocol(format!("Failed to read HTML body: {}", e)))?;

                let asset_exts = [
                    ".zip", ".tar.gz", ".tgz", ".dmg", ".exe", ".pkg", ".iso", ".rar", ".7z",
                    ".bin", ".msi", ".pdf", ".mp4", ".mkv", ".tar",
                ];

                // --- CAPTIVE PORTAL INTERCEPTION ---
                let ends_with_asset = asset_exts
                    .iter()
                    .any(|ext| self.options.uri.to_lowercase().ends_with(ext));
                if ends_with_asset {
                    let body_lower = body.to_lowercase();
                    let keywords = [
                        "login",
                        "signin",
                        "captive",
                        "wifi",
                        "portal",
                        "hotspot",
                        "accept terms",
                        "gateway",
                    ];
                    let has_captive_keyword = keywords.iter().any(|&kw| body_lower.contains(kw));
                    let url_lower = current_uri.to_lowercase();
                    let has_captive_url = url_lower.contains("login")
                        || url_lower.contains("portal")
                        || url_lower.contains("wifi")
                        || url_lower.contains("captive");

                    if has_captive_keyword || has_captive_url {
                        return Err(Error::CaptivePortal(format!(
                            "Captive portal landing page detected at {}",
                            current_uri
                        )));
                    }
                }
                // ------------------------------------

                use super::crawler::RecursiveCrawler;
                if let Ok(mut crawler) = RecursiveCrawler::new(&current_uri, 1, true) {
                    crawler.enqueue_links(&current_uri, &body, 0);
                    let mut found = None;
                    while let Some((link_url, _depth)) = crawler.next_url() {
                        let path = link_url.to_lowercase();
                        if asset_exts.iter().any(|ext| path.ends_with(ext)) {
                            found = Some(link_url);
                            break;
                        }
                    }
                    if let Some(link) = found {
                        tracing::info!(from = %current_uri, to = %link, "Resolved landing page direct link via crawler");
                        referer = Some(current_uri);
                        current_uri = link;
                        continue;
                    }
                }

                return Err(Error::Protocol(format!(
                    "URI {} points to an HTML landing page (Content-Type: {}). Direct link resolution failed.",
                    current_uri, ct
                )));
            }

            let mut total_length = response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.split('/').next_back())
                .and_then(|s| s.parse::<u64>().ok());

            if total_length.is_none() {
                total_length = response
                    .headers()
                    .get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok());
            }

            let name = response
                .headers()
                .get(reqwest::header::CONTENT_DISPOSITION)
                .and_then(|h| h.to_str().ok())
                .and_then(parse_content_disposition);

            tracing::info!(%current_uri, ?total_length, ?name, "Metadata resolved successfully");
            let range_supported = response.status() == reqwest::StatusCode::PARTIAL_CONTENT;

            return Ok(Metadata {
                final_uri: current_uri,
                total_length,
                name,
                range_supported,
                padding_ranges: Vec::new(),
            });
        }
    }
}
