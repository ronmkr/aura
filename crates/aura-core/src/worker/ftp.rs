use super::{Metadata, PieceData, ProgressSender, ProtocolWorker, Segment};
use crate::{Error, Result, TaskId};
use async_trait::async_trait;
use bytes::BytesMut;
use suppaftp::tokio::AsyncFtpStream;
use suppaftp::types::FileType;
use tokio::io::AsyncReadExt;
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

        let host = url
            .host_str()
            .ok_or_else(|| Error::Protocol("Missing host in FTP URL".to_string()))?;
        let port = url.port().unwrap_or(21);
        let user = if url.username().is_empty() {
            "anonymous"
        } else {
            url.username()
        };
        let pass = url.password().unwrap_or("anonymous@aura.rs");

        let mut ftp_stream = AsyncFtpStream::connect(format!("{}:{}", host, port))
            .await
            .map_err(|e| Error::Protocol(format!("Failed to connect to FTP: {}", e)))?;

        ftp_stream
            .login(user, pass)
            .await
            .map_err(|e| Error::Protocol(format!("FTP login failed: {}", e)))?;

        ftp_stream
            .transfer_type(FileType::Binary)
            .await
            .map_err(|e| Error::Protocol(format!("Failed to set FTP binary mode: {}", e)))?;

        Ok(ftp_stream)
    }

    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        let mut ftp: AsyncFtpStream = self.connect().await?;
        let url = Url::parse(&self.uri).unwrap();
        let path = url.path().trim_start_matches('/');

        let size = ftp
            .size(path)
            .await
            .map_err(|e| Error::Protocol(format!("Failed to get FTP file size: {}", e)))?;

        let name = url
            .path_segments()
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
    async fn fetch_segment(
        &self,
        _task_id: TaskId,
        segment: Segment,
        progress: Option<ProgressSender>,
    ) -> Result<PieceData> {
        let mut ftp: AsyncFtpStream = self.connect().await?;
        let url = Url::parse(&self.uri).unwrap();
        let path = url.path().trim_start_matches('/');

        // Set restart point for range-based download
        ftp.resume_transfer(segment.offset as usize)
            .await
            .map_err(|e| Error::Protocol(format!("FTP REST failed: {}", e)))?;

        let mut reader = ftp
            .retr_as_stream(path)
            .await
            .map_err(|e| Error::Protocol(format!("FTP RETR failed: {}", e)))?;

        let mut buffer = BytesMut::with_capacity(segment.length as usize);
        let mut total_read = 0;

        while total_read < segment.length {
            let to_read = std::cmp::min(16384, segment.length - total_read);
            let mut chunk = vec![0u8; to_read as usize];
            let n = reader
                .read(&mut chunk)
                .await
                .map_err(|e| Error::Protocol(format!("FTP read error: {}", e)))?;

            if n == 0 {
                break;
            }

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
