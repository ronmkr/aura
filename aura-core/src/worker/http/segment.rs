use super::super::{PieceData, ProgressSender, ProtocolWorker, Segment};
use super::HttpWorker;
use crate::{Error, Result, TaskId};
use async_trait::async_trait;
use bytes::BytesMut;
use futures_util::StreamExt;
use tokio::sync::mpsc;

#[async_trait]
impl ProtocolWorker for HttpWorker {
    async fn fetch_segment(
        &self,
        task_id: TaskId,
        segment: Segment,
        progress: Option<ProgressSender>,
        storage_tx: Option<mpsc::Sender<crate::storage::StorageRequest>>,
        throttler: std::sync::Arc<crate::throttler::Throttler>,
    ) -> Result<PieceData> {
        let mut attempts = 0;
        let max_attempts = self.options.retry_count;

        loop {
            let upgraded_uri = self.upgrade_url(&self.options.uri).await;
            let response_res = self
                .send_request(&upgraded_uri, |client, uri| {
                    let mut request = client.get(uri);
                    if segment.length != u64::MAX {
                        let range_header = format!(
                            "bytes={}-{}",
                            segment.offset,
                            segment.offset + segment.length - 1
                        );
                        request = request.header("Range", range_header);
                    } else if segment.offset > 0 {
                        request = request.header("Range", format!("bytes={}-", segment.offset));
                    }

                    if let Some(ref referer) = self.options.referer {
                        request = request.header("Referer", referer);
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

            match response_res {
                Ok(response) => {
                    let status = response.status();
                    let sent_range = segment.length != u64::MAX || segment.offset > 0;

                    // If we requested a range but the server ignored it (200 OK
                    // instead of 206 Partial Content), the body starts at byte 0
                    // — writing it at segment.offset would silently corrupt the
                    // file. Return a retriable error so the orchestrator can
                    // restart in single-stream mode. (Issue #251, ADR-0059)
                    if sent_range && status == reqwest::StatusCode::OK && segment.offset > 0 {
                        return Err(Error::Protocol(format!(
                            "Server returned 200 OK for a ranged request at offset {}. \
                             Range header was not honoured — refusing to write at wrong \
                             offset to prevent data corruption. Restart in single-stream mode.",
                            segment.offset
                        )));
                    }

                    if status.is_success() {
                        let mut buffer = BytesMut::with_capacity(16384);

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

                                if let Some(ref s_tx) = storage_tx {
                                    let _ = s_tx
                                        .send(crate::storage::StorageRequest::Write {
                                            task_id,
                                            segment: Segment {
                                                offset: segment.offset + bytes_downloaded,
                                                length: take_len as u64,
                                            },
                                            data: BytesMut::from(sub_chunk),
                                            guard: None,
                                            generation: None,
                                        })
                                        .await;
                                } else {
                                    buffer.extend_from_slice(sub_chunk);
                                }

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
                    } else if Self::is_retryable(status) && attempts < max_attempts {
                        attempts += 1;
                        let delay = self.options.retry_delay_secs * (2u64.pow(attempts - 1));
                        tracing::warn!(
                            %task_id,
                            status = %status,
                            attempt = attempts,
                            delay_secs = delay,
                            "Transient HTTP error, retrying"
                        );
                        tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                        continue;
                    } else {
                        return Err(Error::Protocol(format!("HTTP error status: {}", status)));
                    }
                }
                Err(e) if attempts < max_attempts => {
                    attempts += 1;
                    let delay = self.options.retry_delay_secs * (2u64.pow(attempts - 1));
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
        32 // Allow 32 concurrent requests per HttpWorker
    }
}
