use super::super::Metadata;
use super::HttpWorker;
use crate::{Error, Result};

pub(crate) mod content_disposition;
pub(crate) mod interception;

pub(crate) use content_disposition::parse_content_disposition;

#[cfg(test)]
#[path = "../content_disposition_tests.rs"]
mod content_disposition_tests;

impl HttpWorker {
    /// Resolves the final direct link and file size in a single pass.
    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        tracing::debug!(uri = %self.options.uri, "Resolving metadata");
        let mut current_uri = self.options.uri.clone();
        let mut referer: Option<String> = None;
        let mut redirect_count = 0;
        let max_redirects = self.options.max_redirects;

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

                        if let Some(ref etag) = self.options.if_none_match {
                            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
                        }
                        if let Some(ref last_modified) = self.options.if_modified_since {
                            request =
                                request.header(reqwest::header::IF_MODIFIED_SINCE, last_modified);
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
                        if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
                            return Err(Error::NotModified);
                        } else if resp.status().is_success() || resp.status().is_redirection() {
                            break resp;
                        } else if Self::is_retryable(resp.status()) && attempts < max_attempts {
                            attempts += 1;
                            let delay =
                                self.options.http_retry_delay_secs * (2u64.pow(attempts - 1));
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
                        let delay = self.options.http_retry_delay_secs * (2u64.pow(attempts - 1));
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
                let next_uri = interception::handle_html_interception(
                    &self.options.uri,
                    &current_uri,
                    response,
                )
                .await?;
                referer = Some(current_uri);
                current_uri = next_uri;
                continue;
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

            let etag = response
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string());

            let last_modified = response
                .headers()
                .get(reqwest::header::LAST_MODIFIED)
                .and_then(|h| h.to_str().ok())
                .map(|s| s.to_string());

            return Ok(Metadata {
                final_uri: current_uri,
                total_length,
                name,
                range_supported,
                padding_ranges: Vec::new(),
                etag,
                last_modified,
            });
        }
    }
}
