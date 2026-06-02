use super::{Metadata, PieceData, ProgressSender, ProtocolWorker, Segment};
use crate::{Error, Result, TaskId};
use async_trait::async_trait;
use bytes::BytesMut;
use suppaftp::tokio::AsyncNativeTlsConnector;
use suppaftp::tokio::AsyncNativeTlsFtpStream;
use suppaftp::types::FileType;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use url::Url;

/// A specialized worker for the FTP(S) protocol.
pub struct FtpWorker {
    uri: String,
    local_addr: Option<std::net::IpAddr>,
    retry_count: u32,
    retry_delay_secs: u64,
    credential_provider: Option<std::sync::Arc<crate::config::credentials::CredentialProvider>>,
}

impl FtpWorker {
    pub fn new(
        uri: String,
        local_addr: Option<std::net::IpAddr>,
        retry_count: u32,
        retry_delay_secs: u64,
        credential_provider: Option<std::sync::Arc<crate::config::credentials::CredentialProvider>>,
    ) -> Self {
        Self {
            uri,
            local_addr,
            retry_count,
            retry_delay_secs,
            credential_provider,
        }
    }

    async fn connect_once(&self) -> Result<AsyncNativeTlsFtpStream> {
        let url = Url::parse(&self.uri)
            .map_err(|e| Error::Protocol(format!("Invalid FTP URL: {}", e)))?;

        let host = url
            .host_str()
            .ok_or_else(|| Error::Protocol("Missing host in FTP URL".to_string()))?;
        let port = url.port().unwrap_or(21);

        let mut user = if url.username().is_empty() {
            "anonymous"
        } else {
            url.username()
        };
        let mut pass = url.password().unwrap_or("anonymous@aura.rs");

        let mut resolved_user = None;
        let mut resolved_pass = None;

        if (user == "anonymous") && self.credential_provider.is_some() {
            if let Some(ref provider) = self.credential_provider {
                if let Some(creds) = provider.get_credentials(host) {
                    if let Some(u) = &creds.login {
                        resolved_user = Some(u.clone());
                    }
                    if let Some(p) = &creds.password {
                        resolved_pass = Some(p.clone());
                    }
                }
            }
        }

        if let Some(ref u) = resolved_user {
            user = u;
        }
        if let Some(ref p) = resolved_pass {
            pass = p;
        }

        let tcp_stream =
            crate::net_util::logic::connect_tcp_bound_host(host, port, None, self.local_addr, None)
                .await
                .map_err(|e| {
                    Error::Worker(format!("Failed to connect to FTP host {}: {}", host, e))
                })?;

        let mut ftp_stream = AsyncNativeTlsFtpStream::connect_with_stream(tcp_stream)
            .await
            .map_err(|e| Error::Worker(format!("Failed to initialize FTP stream: {}", e)))?;

        // Determine if we should upgrade to TLS.
        let is_ftps = url.scheme() == "ftps";
        let mut should_upgrade = is_ftps;

        if !should_upgrade {
            // Query FEAT to check if the server supports AUTH TLS.
            if let Ok(features) = ftp_stream.feat().await {
                should_upgrade = features.keys().any(|k| {
                    let k_upper = k.to_uppercase();
                    k_upper.contains("AUTH TLS") || k_upper.contains("AUTH") || k_upper == "TLS"
                });
            }
        }

        if should_upgrade {
            let tls_connector = suppaftp::async_native_tls::TlsConnector::new();
            let connector = AsyncNativeTlsConnector::from(tls_connector);
            ftp_stream = ftp_stream
                .into_secure(connector, host)
                .await
                .map_err(|e| Error::Worker(format!("FTP TLS upgrade failed: {}", e)))?;
        }

        ftp_stream
            .login(user, pass)
            .await
            .map_err(|e| Error::Worker(format!("FTP login failed: {}", e)))?;

        ftp_stream
            .transfer_type(FileType::Binary)
            .await
            .map_err(|e| Error::Worker(format!("Failed to set FTP binary mode: {}", e)))?;

        Ok(ftp_stream)
    }

    async fn connect(&self) -> Result<AsyncNativeTlsFtpStream> {
        let mut attempts = 0;
        let max_attempts = self.retry_count;

        loop {
            match self.connect_once().await {
                Ok(stream) => return Ok(stream),
                Err(e) if attempts < max_attempts => {
                    attempts += 1;
                    let exponent = std::cmp::min(attempts - 1, 30);
                    let delay = self.retry_delay_secs * (2u64.pow(exponent));
                    tracing::warn!(
                        error = %e,
                        attempt = attempts,
                        delay_secs = delay,
                        "Transient FTP connection/login error, retrying"
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }
    }

    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        let mut ftp: AsyncNativeTlsFtpStream = self.connect().await?;
        let url = Url::parse(&self.uri)
            .map_err(|e| Error::Protocol(format!("Invalid FTP URL: {}", e)))?;
        let path = url.path().trim_start_matches('/');

        let size = ftp
            .size(path)
            .await
            .map_err(|e| Error::Worker(format!("Failed to get FTP file size: {}", e)))?;

        let name = url
            .path_segments()
            .and_then(|mut s| s.next_back())
            .map(|s| s.to_string());

        let _ = ftp.quit().await;

        Ok(Metadata {
            final_uri: self.uri.clone(),
            total_length: Some(size as u64),
            name,
            range_supported: true,
            padding_ranges: Vec::new(),
        })
    }
}

#[async_trait]
impl ProtocolWorker for FtpWorker {
    async fn fetch_segment(
        &self,
        task_id: TaskId,
        segment: Segment,
        progress: Option<ProgressSender>,
        storage_tx: Option<mpsc::Sender<crate::storage::StorageRequest>>,
        throttler: std::sync::Arc<crate::throttler::Throttler>,
    ) -> Result<PieceData> {
        let mut ftp: AsyncNativeTlsFtpStream = self.connect().await?;
        let url = Url::parse(&self.uri)
            .map_err(|e| Error::Protocol(format!("Invalid FTP URL: {}", e)))?;
        let path = url.path().trim_start_matches('/');

        // Set restart point for range-based download
        ftp.resume_transfer(segment.offset as usize)
            .await
            .map_err(|e| Error::Worker(format!("FTP REST failed: {}", e)))?;

        let mut reader = ftp
            .retr_as_stream(path)
            .await
            .map_err(|e| Error::Worker(format!("FTP RETR failed: {}", e)))?;

        let mut buffer = BytesMut::with_capacity(16384);
        let mut total_read = 0;

        while total_read < segment.length {
            let to_read = std::cmp::min(16384, segment.length - total_read);

            // Admission Control: Wait for bandwidth tokens before reading
            throttler.acquire_download(task_id, to_read).await;

            let mut chunk = vec![0u8; to_read as usize];
            let n = reader
                .read(&mut chunk)
                .await
                .map_err(|e| Error::Worker(format!("FTP read error: {}", e)))?;

            if n == 0 {
                break;
            }

            if let Some(ref s_tx) = storage_tx {
                let _ = s_tx
                    .send(crate::storage::StorageRequest::Write {
                        task_id,
                        segment: Segment {
                            offset: segment.offset + total_read,
                            length: n as u64,
                        },
                        data: BytesMut::from(&chunk[..n]),
                    })
                    .await;
            } else {
                buffer.extend_from_slice(&chunk[..n]);
            }

            total_read += n as u64;

            if let Some(ref p_tx) = progress {
                let _ = p_tx.send(n as u64);
            }
        }

        let _ = ftp.finalize_retr_stream(reader).await;
        let _ = ftp.quit().await;

        Ok(PieceData {
            segment,
            data: buffer,
        })
    }

    fn available_capacity(&self) -> usize {
        1
    }
}

#[cfg(test)]
#[path = "ftp_tests.rs"]
mod tests;
