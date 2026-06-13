use crate::throttler::Throttler;
use crate::worker::builder::WorkerOptions;
use crate::worker::{Metadata, PieceData, ProgressSender, ProtocolWorker, Segment};
use crate::{Error, Result, TaskId};
use async_trait::async_trait;
use bytes::BytesMut;
use std::sync::Arc;
use tokio::sync::mpsc;
use url::Url;

use super::connection::{parse_ybegin, NntpConnection};

pub struct NntpWorker {
    options: WorkerOptions,
}

impl NntpWorker {
    pub fn new(options: WorkerOptions) -> Self {
        Self { options }
    }

    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        let url = Url::parse(&self.options.uri)
            .map_err(|e| Error::Protocol(format!("Invalid NNTP URI: {}", e)))?;
        let mut message_id = url.path().trim_start_matches('/').to_string();
        if !message_id.starts_with('<') {
            message_id = format!("<{}>", message_id);
        }

        let mut conn = NntpConnection::connect(&self.options.uri, &self.options).await?;

        conn.write_all(format!("BODY {}\r\n", message_id).as_bytes())
            .await
            .map_err(|e| Error::Worker(format!("Failed to send BODY: {}", e)))?;
        conn.flush()
            .await
            .map_err(|e| Error::Worker(format!("Failed to flush: {}", e)))?;

        let mut line = String::new();
        conn.read_line(&mut line)
            .await
            .map_err(|e| Error::Worker(format!("Failed to read BODY response: {}", e)))?;

        if !line.starts_with("222") {
            return Err(Error::Protocol(format!(
                "NNTP BODY command failed: {}",
                line.trim()
            )));
        }

        let mut file_name = None;
        let mut file_size = None;

        for _ in 0..50 {
            line.clear();
            let n = conn
                .read_line(&mut line)
                .await
                .map_err(|e| Error::Worker(format!("Failed to read body line: {}", e)))?;
            if n == 0 {
                break;
            }
            let trimmed = line.trim();
            if trimmed == "." {
                break;
            }
            if let Some((name, size)) = parse_ybegin(trimmed) {
                file_name = Some(name);
                file_size = Some(size);
                break;
            }
        }

        let _ = conn.write_all(b"QUIT\r\n").await;
        let _ = conn.flush().await;

        let name = file_name.unwrap_or_else(|| url.path().trim_start_matches('/').to_string());
        Ok(Metadata {
            final_uri: self.options.uri.clone(),
            total_length: file_size,
            name: Some(name),
            range_supported: true,
            padding_ranges: Vec::new(),
            etag: None,
            last_modified: None,
        })
    }
}

#[async_trait]
impl ProtocolWorker for NntpWorker {
    async fn fetch_segment(
        &self,
        task_id: TaskId,
        segment: Segment,
        progress: Option<ProgressSender>,
        storage_client: Option<Arc<dyn crate::storage::StorageDispatch>>,
        throttler: Arc<Throttler>,
    ) -> Result<PieceData> {
        let mut _guard = if let Some(ref gov) = self.options.resource_governor {
            let req_size = if segment.length == u64::MAX {
                65536
            } else {
                segment.length as usize
            };
            while !gov.request_allocation(&self.options.tenant_id, req_size, false) {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            Some(crate::orchestrator::resource_governor::MemoryGuard::new(
                gov.clone(),
                self.options.tenant_id.clone(),
                req_size,
            ))
        } else {
            None
        };

        let url = Url::parse(&self.options.uri)
            .map_err(|e| Error::Protocol(format!("Invalid NNTP URI: {}", e)))?;
        let mut message_id = url.path().trim_start_matches('/').to_string();
        if !message_id.starts_with('<') {
            message_id = format!("<{}>", message_id);
        }

        let mut conn = NntpConnection::connect(&self.options.uri, &self.options).await?;

        conn.write_all(format!("BODY {}\r\n", message_id).as_bytes())
            .await
            .map_err(|e| Error::Worker(format!("Failed to send BODY: {}", e)))?;
        conn.flush()
            .await
            .map_err(|e| Error::Worker(format!("Failed to flush: {}", e)))?;

        let mut line = String::new();
        conn.read_line(&mut line)
            .await
            .map_err(|e| Error::Worker(format!("Failed to read BODY response: {}", e)))?;

        if !line.starts_with("222") {
            return Err(Error::Protocol(format!(
                "NNTP BODY command failed: {}",
                line.trim()
            )));
        }

        let mut decoded_data = Vec::new();
        loop {
            line.clear();
            let n = conn
                .read_line(&mut line)
                .await
                .map_err(|e| Error::Worker(format!("Failed to read NNTP body line: {}", e)))?;
            if n == 0 {
                break;
            }

            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed == "." {
                break;
            }

            throttler.acquire_download(task_id, n as u64).await;
            if let Some(ref p_tx) = progress {
                let _ = p_tx.send(n as u64);
            }

            let mut line_str = trimmed;
            if line_str.starts_with("..") {
                line_str = &line_str[1..];
            }

            if line_str.starts_with("=y") {
                continue;
            }

            let bytes = line_str.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                let b = bytes[i];
                if b == b'=' && i + 1 < bytes.len() {
                    let next_b = bytes[i + 1];
                    decoded_data.push(next_b.wrapping_sub(64).wrapping_sub(42));
                    i += 2;
                } else {
                    decoded_data.push(b.wrapping_sub(42));
                    i += 1;
                }
            }
        }

        let _ = conn.write_all(b"QUIT\r\n").await;
        let _ = conn.flush().await;

        let start = segment.offset as usize;
        let end = if segment.length == u64::MAX {
            decoded_data.len()
        } else {
            std::cmp::min(decoded_data.len(), start + segment.length as usize)
        };

        let sliced_data = if start < decoded_data.len() {
            &decoded_data[start..end]
        } else {
            &[]
        };

        if let Some(ref s_client) = storage_client {
            let _ = s_client
                .submit_write(
                    task_id,
                    Segment {
                        offset: segment.offset,
                        length: decoded_data.len() as u64,
                    },
                    decoded_data.into(),
                    None,
                    None,
                )
                .await;
        }

        let buffer = if storage_client.is_some() {
            BytesMut::new()
        } else {
            BytesMut::from(sliced_data)
        };

        Ok(PieceData {
            segment: Segment {
                offset: segment.offset,
                length: sliced_data.len() as u64,
            },
            data: buffer,
        })
    }

    fn available_capacity(&self) -> usize {
        1
    }
}
