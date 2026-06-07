use super::{Metadata, PieceData, ProgressSender, ProtocolWorker, Segment};
use crate::{Error, Result, TaskId};
use async_trait::async_trait;
use bytes::BytesMut;
use rustls::ClientConfig;
use rustls::RootCertStore;
use suppaftp::tokio::{AsyncDataStream, AsyncRustlsConnector, AsyncRustlsFtpStream};
use suppaftp::types::FileType;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;
use url::Url;

/// Builds a `rustls` `ClientConfig` seeded from the OS native certificate store.
///
/// Uses `rustls-native-certs` to load platform certificates. Individual certificates
/// that rustls cannot parse are silently skipped — the OS store may contain expired
/// or intermediate certificates that are rejected by rustls's strict DER parser.
/// An empty root store will cause TLS handshakes to untrusted servers to fail, which
/// is the correct secure-by-default behaviour.
fn build_tls_config() -> Result<std::sync::Arc<ClientConfig>> {
    let mut root_store = RootCertStore::empty();

    let cert_result = rustls_native_certs::load_native_certs();

    // Log any load errors but continue — partial cert stores are better than none.
    for error in &cert_result.errors {
        tracing::warn!(%error, "Skipping native TLS certificate that failed to load");
    }

    for cert in cert_result.certs {
        // Silently ignore individual certs that fail to parse.
        let _ = root_store.add(cert);
    }

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Ok(std::sync::Arc::new(config))
}

/// A protocol worker that downloads segments over FTP (plain) or FTPS (explicit TLS
/// via STARTTLS / AUTH TLS). TLS is backed by `rustls` with the `ring` crypto provider
/// and the OS native certificate store via `rustls-native-certs`.
///
/// Both plain and TLS connections are represented by `AsyncRustlsFtpStream`:
/// the internal `DataStream` within suppaftp stores either a raw TCP stream or a TLS
/// stream, allowing the same type alias to cover both cases. TLS is only negotiated
/// when the scheme is `ftps` or when the server advertises `AUTH TLS` in FEAT.
pub struct FtpWorker {
    uri: String,
    local_addr: Option<std::net::IpAddr>,
    retry_count: u32,
    retry_delay_secs: u64,
    happy_eyeballs_stagger_ms: u64,
    credential_provider: Option<std::sync::Arc<crate::config::credentials::CredentialProvider>>,
    pub(crate) resource_governor:
        Option<std::sync::Arc<crate::orchestrator::resource_governor::ResourceGovernor>>,
    pub(crate) tenant_id: Option<crate::TenantId>,
}

impl FtpWorker {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        uri: String,
        local_addr: Option<std::net::IpAddr>,
        retry_count: u32,
        retry_delay_secs: u64,
        happy_eyeballs_stagger_ms: u64,
        credential_provider: Option<std::sync::Arc<crate::config::credentials::CredentialProvider>>,
        resource_governor: Option<
            std::sync::Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
        >,
        tenant_id: Option<crate::TenantId>,
    ) -> Self {
        Self {
            uri,
            local_addr,
            retry_count,
            retry_delay_secs,
            happy_eyeballs_stagger_ms,
            credential_provider,
            resource_governor,
            tenant_id,
        }
    }

    /// Resolves FTP credentials for `host` by consulting first the URL inline
    /// credentials, then the credential provider.  Returns `(user, pass)` as
    /// owned `String`s to sidestep lifetime issues with temporaries.
    fn resolve_credentials(&self, url: &Url, host: &str) -> (String, String) {
        let url_user = url.username();
        let url_pass = url.password().unwrap_or("anonymous@aura.rs");

        // Explicit non-anonymous URL credentials take priority.
        if !url_user.is_empty() && url_user != "anonymous" {
            return (url_user.to_owned(), url_pass.to_owned());
        }

        // Fall back to the credential provider (e.g. ~/.netrc).
        if let Some(ref provider) = self.credential_provider {
            if let Some(creds) = provider.get_credentials(host) {
                let user = creds.login.as_deref().unwrap_or("anonymous").to_owned();
                let pass = creds
                    .password
                    .as_deref()
                    .unwrap_or("anonymous@aura.rs")
                    .to_owned();
                return (user, pass);
            }
        }

        ("anonymous".to_owned(), "anonymous@aura.rs".to_owned())
    }

    /// Establishes a single FTP(S) connection without retry.
    ///
    /// The function always connects using `AsyncRustlsFtpStream::connect_with_stream`,
    /// which internally wraps the TCP stream in suppaftp's `DataStream::Tcp` variant.
    /// If TLS is required, `into_secure` is called to upgrade the control channel to
    /// `DataStream::Ssl`, backed by the rustls `AsyncRustlsConnector`.
    async fn connect_once(&self) -> Result<AsyncRustlsFtpStream> {
        let url = Url::parse(&self.uri)
            .map_err(|e| Error::Protocol(format!("Invalid FTP URL: {}", e)))?;

        let host = url
            .host_str()
            .ok_or_else(|| Error::Protocol("Missing host in FTP URL".to_string()))?;
        let port = url.port().unwrap_or(21);
        let (user, pass) = self.resolve_credentials(&url, host);

        let tcp_stream = crate::net_util::logic::connect_tcp_bound_host(
            host,
            port,
            None,
            self.local_addr,
            None,
            self.happy_eyeballs_stagger_ms,
        )
        .await
        .map_err(|e| Error::Worker(format!("Failed to connect to FTP host {}: {}", host, e)))?;

        // Initialise the FTP control channel over the raw TCP stream.
        // `AsyncRustlsFtpStream` stores `DataStream::Tcp(stream)` internally until
        // `into_secure` is called; no TLS is negotiated yet.
        let mut ftp_stream = AsyncRustlsFtpStream::connect_with_stream(tcp_stream)
            .await
            .map_err(|e| Error::Worker(format!("Failed to initialize FTP stream: {}", e)))?;

        // Determine whether the control channel should be upgraded to TLS.
        //
        // 1. Scheme "ftps" → always upgrade (explicit FTPS / STARTTLS).
        // 2. Scheme "ftp"  → probe FEAT; upgrade opportunistically if the server
        //    advertises AUTH TLS.
        let is_ftps = url.scheme() == "ftps";

        let should_upgrade = if is_ftps {
            true
        } else {
            // Opportunistic TLS: treat any FEAT parse failure as "no TLS".
            ftp_stream
                .feat()
                .await
                .map(|features| {
                    features.keys().any(|k| {
                        let k_upper = k.to_uppercase();
                        k_upper.contains("AUTH TLS") || k_upper.contains("AUTH") || k_upper == "TLS"
                    })
                })
                .unwrap_or(false)
        };

        if should_upgrade {
            let tls_config = build_tls_config()?;
            // Build `tokio_rustls::TlsConnector` (an Arc<ClientConfig> wrapper)
            // then wrap it in suppaftp's `AsyncRustlsConnector` adapter.
            let rustls_connector = suppaftp::tokio_rustls::TlsConnector::from(tls_config);
            let connector = AsyncRustlsConnector::from(rustls_connector);

            ftp_stream = ftp_stream
                .into_secure(connector, host)
                .await
                .map_err(|e| Error::Worker(format!("FTP TLS upgrade failed: {}", e)))?;
        }

        ftp_stream
            .login(&user, &pass)
            .await
            .map_err(|e| Error::Worker(format!("FTP login failed: {}", e)))?;

        ftp_stream
            .transfer_type(FileType::Binary)
            .await
            .map_err(|e| Error::Worker(format!("Failed to set FTP binary mode: {}", e)))?;

        Ok(ftp_stream)
    }

    /// Connects to the FTP server with exponential-backoff retry.
    async fn connect(&self) -> Result<AsyncRustlsFtpStream> {
        let mut attempts: u32 = 0;
        let max_attempts = self.retry_count;

        loop {
            match self.connect_once().await {
                Ok(stream) => return Ok(stream),
                Err(e) if attempts < max_attempts => {
                    attempts += 1;
                    let exponent = std::cmp::min(attempts - 1, 30);
                    let delay = self.retry_delay_secs.saturating_mul(2u64.pow(exponent));
                    tracing::warn!(
                        error = %e,
                        attempt = attempts,
                        max_attempts = max_attempts,
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
        let mut ftp = self.connect().await?;
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
            etag: None,
            last_modified: None,
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
        let mut _guard = if let Some(ref gov) = self.resource_governor {
            let req_size = if segment.length == u64::MAX {
                65536
            } else {
                segment.length as usize
            };
            while !gov.request_allocation(&self.tenant_id, req_size, false) {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Some(crate::orchestrator::resource_governor::MemoryGuard::new(
                gov.clone(),
                self.tenant_id.clone(),
                req_size,
            ))
        } else {
            None
        };

        let mut ftp = self.connect().await?;
        let url = Url::parse(&self.uri)
            .map_err(|e| Error::Protocol(format!("Invalid FTP URL: {}", e)))?;
        let path = url.path().trim_start_matches('/');

        // Set the restart offset for range-based downloads.
        ftp.resume_transfer(segment.offset as usize)
            .await
            .map_err(|e| Error::Worker(format!("FTP REST failed: {}", e)))?;

        let mut reader: AsyncDataStream<suppaftp::tokio::AsyncRustlsStream> = ftp
            .retr_as_stream(path)
            .await
            .map_err(|e| Error::Worker(format!("FTP RETR failed: {}", e)))?;

        let mut buffer = BytesMut::with_capacity(16384);
        let mut total_read: u64 = 0;

        while total_read < segment.length {
            let to_read = std::cmp::min(16384u64, segment.length - total_read) as usize;

            // Admission control: wait for bandwidth tokens before reading.
            throttler.acquire_download(task_id, to_read as u64).await;

            let mut chunk = vec![0u8; to_read];
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
                        guard: None,
                        generation: None,
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
