use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use crate::{Result, TaskId, Error};
use tracing::{info, debug};
use futures_util::StreamExt;

/// Represents a range of bytes to be fetched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    pub offset: u64,
    pub length: u64,
}

/// Represents the data returned by a worker.
#[derive(Debug)]
pub struct PieceData {
    pub segment: Segment,
    pub data: Bytes,
}

/// A sender for progress updates (bytes received).
pub type ProgressSender = tokio::sync::mpsc::UnboundedSender<u64>;

/// The core trait for all network protocols.
#[async_trait]
pub trait ProtocolWorker: Send + Sync {
    async fn fetch_segment(&self, task_id: TaskId, segment: Segment, progress: Option<ProgressSender>) -> Result<PieceData>;
    fn available_capacity(&self) -> usize;
}

use suppaftp::tokio::AsyncFtpStream;
use suppaftp::types::FileType;
use url::Url;

/// A specialized worker for the FTP(S) protocol.
pub struct FtpWorker {
    uri: String,
    _local_addr: Option<std::net::IpAddr>,
}

impl FtpWorker {
    pub fn new(uri: String, local_addr: Option<std::net::IpAddr>) -> Self {
        Self {
            uri,
            _local_addr: local_addr,
        }
    }

    async fn connect(&self) -> Result<AsyncFtpStream> {
        let url = Url::parse(&self.uri)
            .map_err(|e| Error::Protocol(format!("Invalid FTP URL: {}", e)))?;
        
        let host = url.host_str().ok_or_else(|| Error::Protocol("Missing host in FTP URL".to_string()))?;
        let port = url.port().unwrap_or(21);
        let user = if url.username().is_empty() { "anonymous" } else { url.username() };
        let pass = url.password().unwrap_or("anonymous@aura.rs");

        let mut ftp_stream = AsyncFtpStream::connect(format!("{}:{}", host, port)).await
            .map_err(|e| Error::Protocol(format!("Failed to connect to FTP: {}", e)))?;

        ftp_stream.login(user, pass).await
            .map_err(|e| Error::Protocol(format!("FTP login failed: {}", e)))?;

        ftp_stream.transfer_type(FileType::Binary).await
            .map_err(|e| Error::Protocol(format!("Failed to set FTP binary mode: {}", e)))?;

        Ok(ftp_stream)
    }

    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        let mut ftp: AsyncFtpStream = self.connect().await?;
        let url = Url::parse(&self.uri).unwrap();
        let path = url.path().trim_start_matches('/');
        
        let size = ftp.size(path).await
            .map_err(|e| Error::Protocol(format!("Failed to get FTP file size: {}", e)))?;
            
        let name = url.path_segments()
            .and_then(|mut s| s.next_back())
            .map(|s| s.to_string());

        let _ = ftp.quit().await;

        Ok(Metadata {
            final_uri: self.uri.clone(),
            total_length: Some(size as u64),
            name,
        })
    }
}

#[async_trait]
impl ProtocolWorker for FtpWorker {
    async fn fetch_segment(&self, _task_id: TaskId, segment: Segment, progress: Option<ProgressSender>) -> Result<PieceData> {
        let mut ftp: AsyncFtpStream = self.connect().await?;
        let url = Url::parse(&self.uri).unwrap();
        let path = url.path().trim_start_matches('/');

        // Set restart point for range-based download
        ftp.resume_transfer(segment.offset as usize).await
            .map_err(|e| Error::Protocol(format!("FTP REST failed: {}", e)))?;

        let mut reader = ftp.retr_as_stream(path).await
            .map_err(|e| Error::Protocol(format!("FTP RETR failed: {}", e)))?;
        
        let mut buffer = BytesMut::with_capacity(segment.length as usize);
        let mut total_read = 0;

        while total_read < segment.length {
            let to_read = std::cmp::min(16384, segment.length - total_read);
            let mut chunk = vec![0u8; to_read as usize];
            use tokio::io::AsyncReadExt;
            let n = reader.read(&mut chunk).await
                .map_err(|e| Error::Protocol(format!("FTP read error: {}", e)))?;
            
            if n == 0 { break; }
            
            buffer.extend_from_slice(&chunk[..n]);
            total_read += n as u64;

            if let Some(ref p_tx) = progress {
                let _ = p_tx.send(n as u64);
            }
        }

        let _ = ftp.finalize_retr_stream(reader).await;
        let _ = ftp.quit().await;

        Ok(PieceData {
            segment,
            data: buffer.freeze(),
        })
    }

    fn available_capacity(&self) -> usize {
        1
    }
}

/// A specialized worker for the HTTP(S) protocol.
pub struct HttpWorker {
    client: reqwest::Client,
    uri: String,
}

/// Represents resolved metadata for a URI.
#[derive(Debug, Clone)]
pub struct Metadata {
    pub final_uri: String,
    pub total_length: Option<u64>,
    pub name: Option<String>,
}

impl HttpWorker {
    pub fn new(uri: String, local_addr: Option<std::net::IpAddr>) -> Self {
        let cookie_jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
        let mut builder = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Aura/0.1.0")
            .cookie_provider(cookie_jar)
            .redirect(reqwest::redirect::Policy::none());

        if let Some(addr) = local_addr {
            builder = builder.local_address(addr);
        }

        let client = builder.build()
            .expect("Failed to build HTTP client");
            
        Self {
            client,
            uri,
        }
    }

    /// Resolves the final direct link and file size in a single pass.
    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        let mut current_uri = self.uri.clone();
        let mut referer: Option<String> = None;
        let mut redirect_count = 0;
        let max_redirects = 10;

        loop {
            let mut request = self.client.get(&current_uri).header("Range", "bytes=0-0");
            if let Some(ref ref_uri) = referer {
                request = request.header("Referer", ref_uri);
            }

            let response = request.send().await
                .map_err(|e| Error::Protocol(format!("Network error during resolution: {}", e)))?;

            let status = response.status();
            
            if status.is_redirection() {
                redirect_count += 1;
                if redirect_count > max_redirects {
                    return Err(Error::Protocol("Too many redirects during resolution".to_string()));
                }

                let next_uri = response.headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|h| h.to_str().ok())
                    .ok_or_else(|| Error::Protocol("Redirect missing Location header".to_string()))?;
                
                let base = url::Url::parse(&current_uri)
                    .map_err(|e| Error::Protocol(format!("Invalid base URL: {}", e)))?;
                let resolved_next = base.join(next_uri)
                    .map_err(|e| Error::Protocol(format!("Invalid redirect URL: {}", e)))?
                    .to_string();
                
                referer = Some(current_uri);
                current_uri = resolved_next;
                continue;
            }

            if !status.is_success() {
                return Err(Error::Protocol(format!("Server returned error during resolution: {}", status)));
            }

            // Check if it's HTML (Landing Page)
            let mut is_html = false;
            if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
                if let Ok(ct_str) = content_type.to_str() {
                    if ct_str.to_lowercase().contains("text/html") {
                        is_html = true;
                    }
                }
            }

            if is_html {
                let data = response.bytes().await
                    .map_err(|e| Error::Protocol(format!("Failed to read landing page: {}", e)))?;
                let body_str = String::from_utf8_lossy(&data[..std::cmp::min(data.len(), 256 * 1024)]).to_lowercase();

                if let Some(resolved_uri) = find_direct_link(&body_str, &current_uri) {
                    if resolved_uri != current_uri {
                        referer = Some(current_uri);
                        current_uri = resolved_uri;
                        redirect_count += 1;
                        continue;
                    }
                }
                return Err(Error::Protocol("Stuck on landing page during resolution".to_string()));
            }

            // Found the direct link! Extract size from Content-Range or Content-Length
            let total_length = response.headers().get(reqwest::header::CONTENT_RANGE)
                .and_then(|h| h.to_str().ok())
                .and_then(|range| {
                    let slash_pos = range.find('/')?;
                    range[slash_pos + 1..].parse::<u64>().ok()
                })
                .or_else(|| {
                    response.headers().get(reqwest::header::CONTENT_LENGTH)
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                });

            return Ok(Metadata {
                final_uri: current_uri,
                total_length,
                name: None,
            });
        }
    }

    async fn perform_request_and_verify(&self, segment: Segment, progress: Option<ProgressSender>) -> Result<PieceData> {
        let mut current_uri = self.uri.clone();
        let mut referer: Option<String> = None;
        let mut redirect_count = 0;
        let max_redirects = 10;

        loop {
            let range = format!("bytes={}-{}", segment.offset, segment.offset + segment.length - 1);
            debug!(uri = %current_uri, %range, "Fetching segment");

            let mut request = self.client.get(&current_uri).header("Range", &range);
            
            // PROPAGATE REFERER (Iron-Clad State)
            if let Some(ref ref_uri) = referer {
                request = request.header("Referer", ref_uri);
            }

            let response = request.send().await
                .map_err(|e| Error::Protocol(format!("Network error: {}", e)))?;

            let status = response.status();
            
            if status.is_redirection() {
                redirect_count += 1;
                if redirect_count > max_redirects {
                    return Err(Error::Protocol("Too many redirects (loop detected)".to_string()));
                }

                let next_uri = response.headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|h| h.to_str().ok())
                    .ok_or_else(|| Error::Protocol("Redirect missing Location header".to_string()))?;
                
                let base = url::Url::parse(&current_uri)
                    .map_err(|e| Error::Protocol(format!("Invalid base URL: {}", e)))?;
                let resolved_next = base.join(next_uri)
                    .map_err(|e| Error::Protocol(format!("Invalid redirect URL: {}", e)))?
                    .to_string();
                
                debug!(%redirect_count, from = %current_uri, to = %resolved_next, "Following redirect");
                
                // Update state for next hop
                referer = Some(current_uri);
                current_uri = resolved_next;
                continue;
            }

            if !status.is_success() {
                return Err(Error::Protocol(format!("Server returned error: {}", status)));
            }

            // IRON-CLAD MIME VALIDATION (Header based)
            let mut is_html = false;
            let mut mime_str = String::new();

            if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
                if let Ok(ct_str) = content_type.to_str() {
                    mime_str = ct_str.to_lowercase();
                    if mime_str.contains("text/html") {
                        is_html = true;
                    }
                }
            }

            // If it's HTML, we might need to resolve a landing page.
            if is_html {
                let data = response.bytes().await
                    .map_err(|e| Error::Protocol(format!("Failed to read body: {}", e)))?;

                let body_str = String::from_utf8_lossy(&data[..std::cmp::min(data.len(), 256 * 1024)]).to_lowercase();

                debug!(%current_uri, "Hit a landing page, attempting to resolve direct link...");
                if let Some(resolved_uri) = find_direct_link(&body_str, &current_uri) {
                    if resolved_uri != current_uri {
                        info!(from = %current_uri, to = %resolved_uri, "Resolved direct link from landing page");
                        referer = Some(current_uri);
                        current_uri = resolved_uri;
                        redirect_count += 1;
                        continue;
                    }
                }
                return Err(Error::Protocol(format!("Invalid MIME type: {}. Stuck on landing page.", mime_str)));
            }

            // If not HTML, we stream the body and report progress
            let mut stream = response.bytes_stream();
            let mut body_data = BytesMut::with_capacity(segment.length as usize);
            
            while let Some(chunk_result) = stream.next().await {
                let chunk = chunk_result.map_err(|e| Error::Protocol(format!("Stream error: {}", e)))?;
                
                // Progress Reporting
                if let Some(ref p_tx) = progress {
                    let _ = p_tx.send(chunk.len() as u64);
                }
                
                body_data.extend_from_slice(&chunk);
            }

            let data = body_data.freeze();

            // IRON-CLAD CONTENT SNIFFING (Body based fallback)
            if data.len() > 5 {
                let prefix = String::from_utf8_lossy(&data[..std::cmp::min(data.len(), 512)]).to_lowercase();
                if prefix.contains("<!doctype html") || prefix.contains("<html") {
                    return Err(Error::Protocol("Detected HTML content in binary stream.".to_string()));
                }
            }

            // Final safety check: if we had a suspicious MIME and sniffing didn't reveal a known binary format,
            // we could be stricter, but for now we trust the sniffing result.
            
            return Ok(PieceData { segment, data });
        }
    }
}

/// Simple heuristic to extract a download link from an HTML snippet.
fn find_direct_link(html: &str, base_uri: &str) -> Option<String> {
    let base = url::Url::parse(base_uri).ok()?;

    // Heuristic: Find all hrefs and pick the one that looks most like a download link.
    // We look for 'href="' and extract the link until the next '"'.
    let mut best_link = None;
    let mut current_pos = 0;

    while let Some(pos) = html[current_pos..].find("href=\"") {
        let start = current_pos + pos + 6;
        if let Some(end_rel) = html[start..].find('\"') {
            let end = start + end_rel;
            let link = &html[start..end];
            current_pos = end;

            // Parse the link to check its components
            if let Ok(resolved) = base.join(link) {
                let resolved_str = resolved.to_string();
                let path_and_query = format!("{}{}", resolved.path(), resolved.query().unwrap_or(""));
                let path_lower = path_and_query.to_lowercase();

                // Blacklist common non-download assets
                if path_lower.ends_with(".svg") || path_lower.ends_with(".png") || path_lower.ends_with(".css") || path_lower.ends_with(".js") || path_lower.contains("favicon") {
                    continue;
                }

                // Look for keywords in the path/query (not the domain)
                if path_lower.contains("download") || path_lower.contains("file") || path_lower.contains("get") {
                    // Favor links with 'download' or 'file' in them
                    best_link = Some(resolved_str);
                    // If we find 'download', it's a very strong candidate, so we can stop.
                    if path_lower.contains("download") {
                        break;
                    }
                }
            }
        } else {
            break;
        }
    }
    best_link
}
#[async_trait]
impl ProtocolWorker for HttpWorker {
    async fn fetch_segment(&self, _task_id: TaskId, segment: Segment, progress: Option<ProgressSender>) -> Result<PieceData> {
        self.perform_request_and_verify(segment, progress).await
    }

    fn available_capacity(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, header_exists};

    #[tokio::test]
    async fn test_http_worker_referer_propagation() {
        let server = MockServer::start().await;
        
        // Mock 1: Initial request redirects to landing page
        Mock::given(method("GET"))
            .and(path("/start"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "/landing"))
            .mount(&server)
            .await;

        // Mock 2: Landing page requires Referer: /start
        Mock::given(method("GET"))
            .and(path("/landing"))
            .and(header_exists("Referer"))
            .respond_with(ResponseTemplate::new(200).set_body_string("binary_data"))
            .mount(&server)
            .await;

        let worker = HttpWorker::new(format!("{}/start", server.uri()), None);
        let result = worker.fetch_segment(TaskId(1), Segment { offset: 0, length: 11 }, None).await;
        
        assert!(result.is_ok(), "Worker should propagate referer and succeed");
    }

    #[tokio::test]
    async fn test_http_worker_redirect_loop() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/a")).respond_with(ResponseTemplate::new(302).insert_header("Location", "/b")).mount(&server).await;
        Mock::given(method("GET")).and(path("/b")).respond_with(ResponseTemplate::new(302).insert_header("Location", "/a")).mount(&server).await;

        let worker = HttpWorker::new(format!("{}/a", server.uri()), None);
        let result = worker.fetch_segment(TaskId(1), Segment { offset: 0, length: 10 }, None).await;
        match result {
            Err(Error::Protocol(msg)) => assert!(msg.to_lowercase().contains("redirect")),
            _ => panic!("Expected redirect loop error"),
        }
    }
}
