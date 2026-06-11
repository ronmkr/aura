use crate::worker::builder::WorkerOptions;
use crate::{Error, Result};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use url::Url;

use tokio_rustls::rustls::ClientConfig;
use tokio_rustls::TlsConnector;

pub enum NntpConnection {
    Plain(BufReader<TcpStream>),
    Tls(Box<BufReader<tokio_rustls::client::TlsStream<TcpStream>>>),
}

impl NntpConnection {
    pub async fn read_line(&mut self, buf: &mut String) -> std::io::Result<usize> {
        match self {
            NntpConnection::Plain(r) => r.read_line(buf).await,
            NntpConnection::Tls(r) => r.read_line(buf).await,
        }
    }

    pub async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        match self {
            NntpConnection::Plain(r) => r.get_mut().write_all(buf).await,
            NntpConnection::Tls(r) => r.get_mut().write_all(buf).await,
        }
    }

    pub async fn flush(&mut self) -> std::io::Result<()> {
        match self {
            NntpConnection::Plain(r) => r.get_mut().flush().await,
            NntpConnection::Tls(r) => r.get_mut().flush().await,
        }
    }

    pub async fn connect(uri: &str, options: &WorkerOptions) -> Result<Self> {
        let url =
            Url::parse(uri).map_err(|e| Error::Protocol(format!("Invalid NNTP URL: {}", e)))?;

        let host = url
            .host_str()
            .ok_or_else(|| Error::Protocol("Missing host in NNTP URL".to_string()))?;

        let is_tls = url.scheme() == "nntps" || url.port() == Some(563);
        let port = url.port().unwrap_or(if is_tls { 563 } else { 119 });

        let mut user = url.username().to_string();
        let mut pass = url.password().unwrap_or("").to_string();

        if user.is_empty() {
            if let Some(ref provider) = options.credential_provider {
                if let Some(creds) = provider.get_credentials(host) {
                    user = creds.login.clone().unwrap_or_default();
                    pass = creds.password.clone().unwrap_or_default();
                }
            }
        }

        let tcp_stream = crate::net_util::logic::connect_tcp_bound_host(
            host,
            port,
            None,
            options.local_addr,
            None,
            options.happy_eyeballs_stagger_ms,
        )
        .await
        .map_err(|e| Error::Worker(format!("Failed to connect to NNTP host {}: {}", host, e)))?;

        let conn = if is_tls {
            let mut root_store = tokio_rustls::rustls::RootCertStore::empty();
            let cert_result = rustls_native_certs::load_native_certs();
            for error in &cert_result.errors {
                tracing::warn!(%error, "Skipping native TLS certificate that failed to load");
            }
            for cert in cert_result.certs {
                let _ = root_store.add(cert);
            }
            let config = ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth();
            let connector = TlsConnector::from(Arc::new(config));
            let domain = tokio_rustls::rustls::pki_types::ServerName::try_from(host.to_string())
                .map_err(|e| Error::Protocol(format!("Invalid DNS name: {}", e)))?;
            let tls_stream = connector
                .connect(domain, tcp_stream)
                .await
                .map_err(|e| Error::Worker(format!("NNTP TLS handshake failed: {}", e)))?;
            NntpConnection::Tls(Box::new(BufReader::new(tls_stream)))
        } else {
            NntpConnection::Plain(BufReader::new(tcp_stream))
        };

        let mut conn = conn;
        let mut line = String::new();
        conn.read_line(&mut line)
            .await
            .map_err(|e| Error::Worker(format!("Failed to read NNTP welcome banner: {}", e)))?;

        if !line.starts_with("200") && !line.starts_with("201") {
            return Err(Error::Protocol(format!(
                "NNTP welcome banner error: {}",
                line.trim()
            )));
        }

        if !user.is_empty() {
            conn.write_all(format!("AUTHINFO USER {}\r\n", user).as_bytes())
                .await
                .map_err(|e| Error::Worker(format!("Failed to send AUTHINFO USER: {}", e)))?;
            conn.flush()
                .await
                .map_err(|e| Error::Worker(format!("Failed to flush NNTP stream: {}", e)))?;

            line.clear();
            conn.read_line(&mut line).await.map_err(|e| {
                Error::Worker(format!("Failed to read AUTHINFO USER response: {}", e))
            })?;

            if line.starts_with("381") {
                conn.write_all(format!("AUTHINFO PASS {}\r\n", pass).as_bytes())
                    .await
                    .map_err(|e| Error::Worker(format!("Failed to send AUTHINFO PASS: {}", e)))?;
                conn.flush()
                    .await
                    .map_err(|e| Error::Worker(format!("Failed to flush NNTP stream: {}", e)))?;

                line.clear();
                conn.read_line(&mut line).await.map_err(|e| {
                    Error::Worker(format!("Failed to read AUTHINFO PASS response: {}", e))
                })?;
            }

            if !line.starts_with("281") {
                return Err(Error::Protocol(format!(
                    "NNTP authentication failed: {}",
                    line.trim()
                )));
            }
        }

        Ok(conn)
    }
}

pub fn parse_ybegin(line: &str) -> Option<(String, u64)> {
    if !line.starts_with("=ybegin") {
        return None;
    }
    let mut name = None;
    let mut size = None;

    if let Some(name_pos) = line.find("name=") {
        name = Some(line[name_pos + 5..].trim().to_string());
    }

    for part in line.split_whitespace() {
        if let Some(stripped) = part.strip_prefix("size=") {
            if let Ok(s) = stripped.parse::<u64>() {
                size = Some(s);
            }
        }
    }

    if let (Some(n), Some(s)) = (name, size) {
        Some((n, s))
    } else {
        None
    }
}
