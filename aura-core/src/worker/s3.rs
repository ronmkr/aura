use crate::throttler::Throttler;
use crate::worker::builder::WorkerOptions;
use crate::worker::{Metadata, PieceData, ProgressSender, ProtocolWorker, Segment};
use crate::{Error, Result, TaskId};
use async_trait::async_trait;
use std::sync::Arc;
#[cfg(feature = "s3")]
use tokio::sync::OnceCell;

pub struct S3Worker {
    #[allow(dead_code)]
    options: WorkerOptions,
    #[cfg(feature = "s3")]
    client: OnceCell<aws_sdk_s3::Client>,
}

impl S3Worker {
    pub fn new(options: WorkerOptions) -> Self {
        Self {
            options,
            #[cfg(feature = "s3")]
            client: OnceCell::new(),
        }
    }

    #[cfg(feature = "s3")]
    async fn get_client(&self) -> &aws_sdk_s3::Client {
        self.client
            .get_or_init(|| async {
                let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
                    .load()
                    .await;
                aws_sdk_s3::Client::new(&sdk_config)
            })
            .await
    }

    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        #[cfg(feature = "s3")]
        {
            let url = url::Url::parse(&self.options.uri)
                .map_err(|e| Error::Protocol(format!("Invalid S3 URI: {}", e)))?;
            let bucket = url
                .host_str()
                .ok_or_else(|| Error::Protocol("Missing S3 bucket".to_string()))?;
            let key = url.path().trim_start_matches('/');

            let client = self.get_client().await;
            let response = client
                .head_object()
                .bucket(bucket)
                .key(key)
                .send()
                .await
                .map_err(|e| Error::Worker(format!("S3 head_object failed: {}", e)))?;

            let content_length = response.content_length().map(|l| l as u64);
            let etag = response.e_tag().map(|s| s.to_string());
            let last_modified = response.last_modified().map(|t| t.to_string());

            Ok(Metadata {
                final_uri: self.options.uri.clone(),
                total_length: content_length,
                name: Some(key.split('/').next_back().unwrap_or(key).to_string()),
                range_supported: true,
                padding_ranges: Vec::new(),
                etag,
                last_modified,
            })
        }
        #[cfg(not(feature = "s3"))]
        {
            Err(Error::Protocol("S3 feature not enabled".to_string()))
        }
    }
}

#[async_trait]
impl ProtocolWorker for S3Worker {
    async fn fetch_segment(
        &self,
        task_id: TaskId,
        segment: Segment,
        progress: Option<ProgressSender>,
        storage_client: Option<Arc<dyn crate::storage::StorageDispatch>>,
        throttler: Arc<Throttler>,
    ) -> Result<PieceData> {
        #[cfg(feature = "s3")]
        {
            let url = url::Url::parse(&self.options.uri)
                .map_err(|e| Error::Protocol(format!("Invalid S3 URI: {}", e)))?;
            let bucket = url
                .host_str()
                .ok_or_else(|| Error::Protocol("Missing S3 bucket".to_string()))?;
            let key = url.path().trim_start_matches('/');

            let client = self.get_client().await;
            let mut request = client.get_object().bucket(bucket).key(key);

            if segment.length != u64::MAX {
                let range_header = format!(
                    "bytes={}-{}",
                    segment.offset,
                    segment.offset + segment.length - 1
                );
                request = request.range(range_header);
            } else if segment.offset > 0 {
                request = request.range(format!("bytes={}-", segment.offset));
            }

            let response = request
                .send()
                .await
                .map_err(|e| Error::Worker(format!("S3 get_object failed: {}", e)))?;

            let sent_range = segment.length != u64::MAX || segment.offset > 0;
            if sent_range && response.content_range().is_none() && segment.offset > 0 {
                return Err(Error::Protocol(format!(
                    "S3 server returned response without content-range for a ranged request at offset {}. \
                     Range header was not honoured — refusing to write at wrong offset to prevent data corruption.",
                    segment.offset
                )));
            }

            let mut stream = response.body;
            let buffer_cap = self.options.http_buffer_capacity;
            let mut buffer = bytes::BytesMut::with_capacity(buffer_cap);
            let mut bytes_downloaded = 0u64;

            while let Some(chunk_res) = stream.next().await {
                let chunk =
                    chunk_res.map_err(|e| Error::Protocol(format!("S3 stream error: {}", e)))?;

                let mut remaining_chunk = &chunk[..];
                while !remaining_chunk.is_empty() {
                    if bytes_downloaded >= segment.length {
                        break;
                    }

                    let max_take = (segment.length - bytes_downloaded) as usize;
                    let take_len =
                        std::cmp::min(remaining_chunk.len(), std::cmp::min(buffer_cap, max_take));
                    let sub_chunk = &remaining_chunk[..take_len];

                    throttler.acquire_download(task_id, take_len as u64).await;

                    if let Some(ref s_client) = storage_client {
                        let _ = s_client
                            .submit_write(
                                task_id,
                                Segment {
                                    offset: segment.offset + bytes_downloaded,
                                    length: take_len as u64,
                                },
                                sub_chunk.into(),
                                _guard.clone(),
                                None,
                            )
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

            Ok(PieceData {
                segment,
                data: buffer,
            })
        }
        #[cfg(not(feature = "s3"))]
        {
            let _ = task_id;
            let _ = segment;
            let _ = progress;
            let _ = storage_client;
            let _ = throttler;
            Err(Error::Protocol("S3 feature not enabled".to_string()))
        }
    }

    fn available_capacity(&self) -> usize {
        1
    }
}
