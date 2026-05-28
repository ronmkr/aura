use super::super::{PieceData, ProgressSender, ProtocolWorker, Segment};
use super::HttpWorker;
use crate::{Error, Result, TaskId};
use async_trait::async_trait;
use bytes::BytesMut;
use futures_util::StreamExt;

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
            let upgraded_uri = self.upgrade_url(&self.uri).await;
            let mut request = self.client.get(&upgraded_uri);
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

            if let Some(ref referer) = self.referer {
                request = request.header("Referer", referer);
            }

            if let Some(ref provider) = self.credential_provider {
                if let Ok(url) = url::Url::parse(&upgraded_uri) {
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
                    self.check_and_update_hsts(&response).await;
                    if response.status().is_success() {
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
        32 // Allow 32 concurrent requests per HttpWorker
    }
}
