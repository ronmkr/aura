use crate::{Result, TaskId, Error};
use async_trait::async_trait;
use bytes::BytesMut;
use futures_util::StreamExt;
use super::{ProtocolWorker, Segment, PieceData, Metadata, ProgressSender};

/// A specialized worker for the HTTP(S) protocol.
pub struct HttpWorker {
    client: reqwest::Client,
    uri: String,
    referer: Option<String>,
}

impl HttpWorker {
    pub fn new(
        uri: String, 
        local_addr: Option<std::net::IpAddr>,
        user_agent: Option<String>,
        connect_timeout: Option<u64>,
        proxy: Option<String>,
        referer: Option<String>,
    ) -> Self {
        let cookie_jar = std::sync::Arc::new(reqwest::cookie::Jar::default());
        let mut builder = reqwest::Client::builder()
            .user_agent(user_agent.unwrap_or_else(|| "Aura/0.1.0".to_string()))
            .cookie_provider(cookie_jar)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(connect_timeout.unwrap_or(30)))
            .tcp_keepalive(std::time::Duration::from_secs(60));

        if let Some(addr) = local_addr {
            builder = builder.local_address(addr);
        }

        if let Some(p) = proxy {
            if let Ok(proxy_obj) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy_obj);
            }
        }

        let client = builder.build()
            .expect("Failed to build HTTP client");
            
        Self {
            client,
            uri,
            referer,
        }
    }

    /// Resolves the final direct link and file size in a single pass.
    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        let mut current_uri = self.uri.clone();
        let mut referer: Option<String> = None;
        let mut redirect_count = 0;
        let max_redirects = 20;

        loop {
            let mut request = self.client.get(&current_uri).header("Range", "bytes=0-0");
            if let Some(ref ref_uri) = referer {
                request = request.header("Referer", ref_uri);
            }
            
            let response = request.send().await
                .map_err(|e| Error::Protocol(format!("Metadata resolution failed: {}", e)))?;

            if response.status().is_redirection() {
                if redirect_count >= max_redirects {
                    return Err(Error::Protocol("Too many redirects".to_string()));
                }
                redirect_count += 1;
                
                let next_url = response.headers().get("Location")
                    .and_then(|h| h.to_str().ok())
                    .ok_or_else(|| Error::Protocol("Redirect without Location header".to_string()))?;
                
                let resolved_next = url::Url::parse(&current_uri)
                    .and_then(|base| base.join(next_url))
                    .map_err(|e| Error::Protocol(format!("Invalid redirect URL: {}", e)))?
                    .to_string();
                
                referer = Some(current_uri);
                current_uri = resolved_next;
                continue;
            }

            let total_length = response.headers()
                .get("Content-Range")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.split('/').next_back())
                .and_then(|s| s.parse::<u64>().ok());

            let name = response.headers()
                .get("Content-Disposition")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.find("filename=").map(|pos| s[pos+9..].trim_matches('"').to_string()));

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
    async fn fetch_segment(&self, _task_id: TaskId, segment: Segment, progress: Option<ProgressSender>) -> Result<PieceData> {
        let range_header = format!("bytes={}-{}", segment.offset, segment.offset + segment.length - 1);
        let mut request = self.client.get(&self.uri)
            .header("Range", range_header);

        if let Some(ref ref_uri) = self.referer {
            request = request.header("Referer", ref_uri);
        }

        let response = request.send()
            .await
            .map_err(|e| Error::Protocol(format!("HTTP request failed: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::Protocol(format!("HTTP error status: {}", response.status())));
        }

        let mut buffer = BytesMut::with_capacity(segment.length as usize);
        let mut stream = response.bytes_stream();

        while let Some(chunk_res) = stream.next().await {
            let chunk = chunk_res.map_err(|e| Error::Protocol(format!("Stream error: {}", e)))?;
            buffer.extend_from_slice(&chunk);
            if let Some(ref p_tx) = progress {
                let _ = p_tx.send(chunk.len() as u64);
            }
        }

        Ok(PieceData {
            segment,
            data: buffer.freeze(),
        })
    }

    fn available_capacity(&self) -> usize {
        4 // Allow 4 concurrent requests per HttpWorker
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{MockServer, Mock, ResponseTemplate};
    use wiremock::matchers::{method, path, header};

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

        let worker = HttpWorker::new(format!("{}/start", server.uri()), None, None, None, None, None);
        let metadata = worker.resolve_metadata().await.expect("Should resolve metadata with redirects");
        
        let worker_final = HttpWorker::new(metadata.final_uri, None, None, None, None, Some(format!("{}/start", server.uri())));
        let result = worker_final.fetch_segment(TaskId(1), Segment { offset: 0, length: 11 }, None).await;
        
        assert!(result.is_ok(), "Worker should succeed with resolved URI");
    }

    #[tokio::test]
    async fn test_http_worker_redirect_loop() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/a")).respond_with(ResponseTemplate::new(302).insert_header("Location", "/b")).mount(&server).await;
        Mock::given(method("GET")).and(path("/b")).respond_with(ResponseTemplate::new(302).insert_header("Location", "/a")).mount(&server).await;

        let worker = HttpWorker::new(format!("{}/a", server.uri()), None, None, None, None, None);
        let result = worker.resolve_metadata().await;
        match result {
            Err(Error::Protocol(msg)) => assert!(msg.to_lowercase().contains("redirect")),
            _ => panic!("Expected redirect loop error"),
        }
    }
}
