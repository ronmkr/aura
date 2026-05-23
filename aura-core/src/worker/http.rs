use super::{Metadata, PieceData, ProgressSender, ProtocolWorker, Segment};
use crate::buffer_pool::BufferPool;
use crate::{Error, Result, TaskId};
use async_trait::async_trait;
use bytes::BytesMut;
use futures_util::StreamExt;

/// A specialized worker for the HTTP(S) protocol.
pub struct HttpWorker {
    client: reqwest::Client,
    uri: String,
    referer: Option<String>,
    pool: Option<BufferPool>,
    retry_count: u32,
    retry_delay_secs: u64,
    credential_provider: Option<std::sync::Arc<crate::config::credentials::CredentialProvider>>,
}

impl HttpWorker {
    fn is_retryable(status: reqwest::StatusCode) -> bool {
        status.is_server_error() || status == reqwest::StatusCode::TOO_MANY_REQUESTS
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        uri: String,
        local_addr: Option<std::net::IpAddr>,
        user_agent: Option<String>,
        connect_timeout: Option<u64>,
        proxy: Option<String>,
        referer: Option<String>,
        pool: Option<BufferPool>,
        retry_count: u32,
        retry_delay_secs: u64,
        credential_provider: Option<std::sync::Arc<crate::config::credentials::CredentialProvider>>,
    ) -> Self {
        let cookie_jar = if let Some(ref provider) = credential_provider {
            provider.cookie_jar()
        } else {
            std::sync::Arc::new(reqwest::cookie::Jar::default())
        };

        let mut builder = reqwest::Client::builder()
            .user_agent(user_agent.unwrap_or_else(|| "Aura/0.1.0".to_string()))
            .cookie_provider(cookie_jar)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(
                connect_timeout.unwrap_or(30),
            ))
            .tcp_keepalive(std::time::Duration::from_secs(60));

        if let Some(addr) = local_addr {
            builder = builder.local_address(addr);
        }

        if let Some(p) = proxy {
            if let Ok(proxy_obj) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy_obj);
            }
        }

        let client = builder.build().expect("Failed to build HTTP client");

        Self {
            client,
            uri,
            referer,
            pool,
            retry_count,
            retry_delay_secs,
            credential_provider,
        }
    }

    /// Resolves the final direct link and file size in a single pass.
    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        let mut current_uri = self.uri.clone();
        let mut referer: Option<String> = None;
        let mut redirect_count = 0;
        let max_redirects = 20;

        let link_regex = regex::Regex::new(r#"(?i)<a\s+[^>]*href=["']([^"']+)["']"#).unwrap();

        loop {
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

                let base_url = url::Url::parse(&current_uri)
                    .map_err(|e| Error::Protocol(format!("Invalid current URI for base: {}", e)))?;

                let mut resolved_link = None;
                let asset_exts = [
                    ".zip", ".tar.gz", ".tgz", ".dmg", ".exe", ".pkg", ".iso", ".rar", ".7z",
                    ".bin", ".msi", ".pdf", ".mp4", ".mkv", ".tar",
                ];

                for cap in link_regex.captures_iter(&body) {
                    let href = &cap[1];
                    if let Ok(resolved) = base_url.join(href) {
                        let path = resolved.path().to_lowercase();
                        if asset_exts.iter().any(|ext| path.ends_with(ext)) {
                            resolved_link = Some(resolved.to_string());
                            break;
                        }
                    }
                }

                if let Some(link) = resolved_link {
                    tracing::info!(from = %current_uri, to = %link, "Resolved landing page direct link");
                    referer = Some(current_uri);
                    current_uri = link;
                    continue;
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

            return Ok(Metadata {
                final_uri: current_uri,
                total_length,
                name,
            });
        }
    }
}

#[async_trait]
impl ProtocolWorker for HttpWorker {
    async fn fetch_segment(
        &self,
        task_id: TaskId,
        segment: Segment,
        progress: Option<ProgressSender>,
        throttler: std::sync::Arc<crate::throttler::Throttler>,
    ) -> Result<PieceData> {
        let mut attempts = 0;
        let max_attempts = self.retry_count;

        loop {
            let range_header = format!(
                "bytes={}-{}",
                segment.offset,
                segment.offset + segment.length - 1
            );
            let mut request = self.client.get(&self.uri).header("Range", range_header);

            if let Some(ref ref_uri) = self.referer {
                request = request.header("Referer", ref_uri);
            }

            if let Some(ref provider) = self.credential_provider {
                if let Ok(url) = url::Url::parse(&self.uri) {
                    if let Some(host) = url.host_str() {
                        if let Some(creds) = provider.get_credentials(host) {
                            if let (Some(user), Some(pass)) = (&creds.login, &creds.password) {
                                request = request.basic_auth(user, Some(pass));
                            }
                        }
                    }
                }
            }

            let response_res = request.send().await;

            match response_res {
                Ok(response) => {
                    if response.status().is_success() {
                        let mut buffer = if let Some(ref p) = self.pool {
                            p.acquire()
                        } else {
                            BytesMut::with_capacity(segment.length as usize)
                        };

                        let mut stream = response.bytes_stream();
                        let mut bytes_downloaded = 0u64;

                        while let Some(chunk_res) = stream.next().await {
                            let chunk = chunk_res
                                .map_err(|e| Error::Protocol(format!("Stream error: {}", e)))?;

                            let mut remaining_chunk = &chunk[..];
                            while !remaining_chunk.is_empty() {
                                if bytes_downloaded >= segment.length {
                                    break;
                                }

                                let max_take = (segment.length - bytes_downloaded) as usize;
                                let take_len = std::cmp::min(
                                    remaining_chunk.len(),
                                    std::cmp::min(16384, max_take),
                                );
                                let sub_chunk = &remaining_chunk[..take_len];

                                throttler.acquire_download(task_id, take_len as u64).await;

                                buffer.extend_from_slice(sub_chunk);
                                bytes_downloaded += take_len as u64;
                                if let Some(ref p_tx) = progress {
                                    let _ = p_tx.send(take_len as u64);
                                }

                                remaining_chunk = &remaining_chunk[take_len..];
                            }

                            if bytes_downloaded >= segment.length {
                                break;
                            }
                        }

                        return Ok(PieceData {
                            segment,
                            data: buffer,
                        });
                    } else if Self::is_retryable(response.status()) && attempts < max_attempts {
                        attempts += 1;
                        let delay = self.retry_delay_secs * (2u64.pow(attempts - 1));
                        tracing::warn!(
                            %task_id,
                            status = %response.status(),
                            attempt = attempts,
                            delay_secs = delay,
                            "Transient HTTP error, retrying"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                        continue;
                    } else {
                        return Err(Error::Protocol(format!(
                            "HTTP error status: {}",
                            response.status()
                        )));
                    }
                }
                Err(e) if attempts < max_attempts => {
                    attempts += 1;
                    let delay = self.retry_delay_secs * (2u64.pow(attempts - 1));
                    tracing::warn!(
                        %task_id,
                        error = %e,
                        attempt = attempts,
                        delay_secs = delay,
                        "HTTP request failed, retrying"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    continue;
                }
                Err(e) => return Err(Error::Worker(format!("HTTP request failed: {}", e))),
            }
        }
    }

    fn available_capacity(&self) -> usize {
        4 // Allow 4 concurrent requests per HttpWorker
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn test_http_worker_referer_propagation() {
        let server = MockServer::start().await;

        // 1. Initial request redirects to 2
        Mock::given(method("GET"))
            .and(path("/start"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "/final"))
            .mount(&server)
            .await;

        // 2. Second request must have Referer: /start
        Mock::given(method("GET"))
            .and(path("/final"))
            .and(header("Referer", &format!("{}/start", server.uri())))
            .respond_with(ResponseTemplate::new(200).set_body_string("binary_data"))
            .mount(&server)
            .await;

        let worker = HttpWorker::new(
            format!("{}/start", server.uri()),
            None,
            None,
            None,
            None,
            None,
            None,
            5,
            2,
            None,
        );
        let metadata = worker
            .resolve_metadata()
            .await
            .expect("Should resolve metadata with redirects");

        let worker_final = HttpWorker::new(
            metadata.final_uri,
            None,
            None,
            None,
            None,
            Some(format!("{}/start", server.uri())),
            None,
            5,
            2,
            None,
        );
        let throttler = std::sync::Arc::new(crate::throttler::Throttler::new(0, 0));
        let result = worker_final
            .fetch_segment(
                TaskId(1),
                Segment {
                    offset: 0,
                    length: 11,
                },
                None,
                throttler,
            )
            .await;

        assert!(result.is_ok(), "Worker should succeed with resolved URI");
    }

    #[tokio::test]
    async fn test_http_worker_redirect_loop() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/a"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "/b"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/b"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "/a"))
            .mount(&server)
            .await;

        let worker = HttpWorker::new(
            format!("{}/a", server.uri()),
            None,
            None,
            None,
            None,
            None,
            None,
            5,
            2,
            None,
        );
        let result = worker.resolve_metadata().await;
        match result {
            Err(Error::Protocol(msg)) => assert!(msg.to_lowercase().contains("redirect")),
            _ => panic!("Expected redirect loop error"),
        }
    }

    #[tokio::test]
    async fn test_http_worker_retry_on_503() {
        let server = MockServer::start().await;
        let counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));

        Mock::given(method("GET"))
            .respond_with(move |_req: &wiremock::Request| {
                let prev = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if prev < 2 {
                    ResponseTemplate::new(503)
                } else {
                    ResponseTemplate::new(200)
                        .set_body_bytes(vec![1u8; 10])
                        .insert_header("Content-Range", "bytes 0-9/10")
                }
            })
            .mount(&server)
            .await;

        let worker = HttpWorker::new(
            format!("{}/retry", server.uri()),
            None,
            None,
            None,
            None,
            None,
            None,
            3, // Max retries
            1, // 1s base delay
            None,
        );

        let throttler = std::sync::Arc::new(crate::throttler::Throttler::new(0, 0));
        let result = worker
            .fetch_segment(
                TaskId(1),
                Segment {
                    offset: 0,
                    length: 10,
                },
                None,
                throttler,
            )
            .await;

        if let Err(ref e) = result {
            panic!("Retry test failed with error: {}", e);
        }
        assert!(result.is_ok());
        assert_eq!(result.unwrap().data.len(), 10);
    }

    #[tokio::test]
    async fn test_http_worker_html_landing_page_resolution_success() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/landing"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/html; charset=UTF-8")
                    .set_body_bytes(
                        "<html><body>Download here: <a href='/download/file.zip'>link</a></body></html>"
                            .as_bytes()
                            .to_vec(),
                    ),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/download/file.zip"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/octet-stream")
                    .set_body_bytes(vec![0u8; 100]),
            )
            .mount(&server)
            .await;

        let worker = HttpWorker::new(
            format!("{}/landing", server.uri()),
            None,
            None,
            None,
            None,
            None,
            None,
            3,
            1,
            None,
        );

        let result = worker.resolve_metadata().await;
        assert!(
            result.is_ok(),
            "Should successfully resolve intermediate landing page: {:?}",
            result.err()
        );
        let meta = result.unwrap();
        assert!(meta.final_uri.contains("/download/file.zip"));
        assert_eq!(meta.total_length, Some(100));
    }

    #[tokio::test]
    async fn test_http_worker_html_landing_page_resolution_failure() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/landing"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "text/html; charset=UTF-8")
                    .set_body_bytes(
                        "<html><body>Welcome to landing page! No direct links here.</body></html>"
                            .as_bytes()
                            .to_vec(),
                    ),
            )
            .mount(&server)
            .await;

        let worker = HttpWorker::new(
            format!("{}/landing", server.uri()),
            None,
            None,
            None,
            None,
            None,
            None,
            3,
            1,
            None,
        );

        let result = worker.resolve_metadata().await;
        assert!(result.is_err());
        match result {
            Err(Error::Protocol(msg)) => assert!(msg.contains("Direct link resolution failed")),
            _ => panic!("Expected Protocol error for HTML landing page failure"),
        }
    }
}
