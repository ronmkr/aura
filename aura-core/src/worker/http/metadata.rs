use super::super::Metadata;
use super::HttpWorker;
use crate::{Error, Result};

impl HttpWorker {
    /// Resolves the final direct link and file size in a single pass.
    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        tracing::debug!(uri = %self.uri, "Resolving metadata");
        let mut current_uri = self.uri.clone();
        let mut referer: Option<String> = None;
        let mut redirect_count = 0;
        let max_redirects = 20;

        loop {
            current_uri = self.upgrade_url(&current_uri).await;
            let mut attempts = 0;
            let max_attempts = self.retry_count;

            let response = loop {
                let mut request = self.client.get(&current_uri).header("Range", "bytes=0-0");
                if let Some(ref ref_uri) = referer {
                    request = request.header("Referer", ref_uri);
                }

                if let Some(ref provider) = self.credential_provider {
                    if let Ok(url) = url::Url::parse(&current_uri) {
                        if let Some(host) = url.host_str() {
                            if let Some(creds) = provider.get_credentials(host) {
                                if let (Some(user), Some(pass)) = (&creds.login, &creds.password) {
                                    request = request.basic_auth(user, Some(pass));
                                }
                            }
                        }
                    }
                }

                let res = request.send().await;

                match res {
                    Ok(resp) => {
                        self.check_and_update_hsts(&resp).await;
                        if resp.status().is_success() || resp.status().is_redirection() {
                            break resp;
                        } else if Self::is_retryable(resp.status()) && attempts < max_attempts {
                            attempts += 1;
                            let delay = self.retry_delay_secs * (2u64.pow(attempts - 1));
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
                        let delay = self.retry_delay_secs * (2u64.pow(attempts - 1));
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
                    .any(|ext| self.uri.to_lowercase().ends_with(ext));
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
                .and_then(|s| {
                    s.find("filename=")
                        .map(|pos| s[pos + 9..].trim_matches('"').to_string())
                });

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
