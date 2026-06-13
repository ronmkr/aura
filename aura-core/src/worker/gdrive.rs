#[cfg(feature = "gdrive")]
use super::gdrive_utils::{extract_file_id, extract_onedrive_id};
use crate::throttler::Throttler;
use crate::worker::builder::WorkerOptions;
use crate::worker::{Metadata, PieceData, ProgressSender, ProtocolWorker, Segment};
use crate::{Error, Result, TaskId};
use async_trait::async_trait;
#[cfg(feature = "gdrive")]
use futures_util::StreamExt;
use std::sync::Arc;

pub struct GDriveWorker {
    #[allow(dead_code)]
    client: Arc<reqwest::Client>,
    #[allow(dead_code)]
    options: WorkerOptions,
}

impl GDriveWorker {
    pub fn new(options: WorkerOptions) -> Self {
        let client = if let Some(ref pool) = options.client_pool {
            if let Some(key) =
                crate::worker::http::ClientKey::from_uri("https://www.googleapis.com")
            {
                pool.get_or_create(&key, || reqwest::Client::builder().build().unwrap())
            } else {
                Arc::new(reqwest::Client::new())
            }
        } else {
            Arc::new(reqwest::Client::new())
        };
        Self { client, options }
    }

    pub async fn resolve_metadata(&self) -> Result<Metadata> {
        #[cfg(feature = "gdrive")]
        {
            if self.options.uri.starts_with("onedrive://")
                || self.options.uri.contains("onedrive.live.com")
                || self.options.uri.contains("sharepoint.com")
            {
                let item_id = extract_onedrive_id(&self.options.uri)
                    .ok_or_else(|| Error::Protocol("Invalid OneDrive URI".to_string()))?;
                let endpoint = std::env::var("AURA_ONEDRIVE_ENDPOINT")
                    .unwrap_or_else(|_| "https://graph.microsoft.com".to_string());
                let url_str = format!("{}/v1.0/me/drive/items/{}", endpoint, item_id);
                let mut request = self.client.get(&url_str);
                if let Some(ref provider) = self.options.credential_provider {
                    if let Some(creds) = provider
                        .get_credentials("graph.microsoft.com")
                        .or_else(|| provider.get_credentials("onedrive.com"))
                    {
                        if let Some(ref token) = creds.password {
                            request = request.header("Authorization", format!("Bearer {}", token));
                        }
                    }
                }
                let response = request.send().await.map_err(|e| {
                    Error::Worker(format!("OneDrive metadata request failed: {}", e))
                })?;
                if !response.status().is_success() {
                    return Err(Error::Protocol(format!(
                        "OneDrive error status: {}",
                        response.status()
                    )));
                }
                #[derive(serde::Deserialize)]
                struct OneDriveMetadata {
                    name: String,
                    size: u64,
                }
                let meta: OneDriveMetadata = response.json().await.map_err(|e| {
                    Error::Protocol(format!("Failed to parse OneDrive metadata: {}", e))
                })?;
                return Ok(Metadata {
                    final_uri: self.options.uri.clone(),
                    total_length: Some(meta.size),
                    name: Some(meta.name),
                    range_supported: true,
                    padding_ranges: Vec::new(),
                    etag: None,
                    last_modified: None,
                });
            }

            let file_id = extract_file_id(&self.options.uri)
                .ok_or_else(|| Error::Protocol("Invalid Google Drive URI".to_string()))?;
            let endpoint = std::env::var("AURA_GDRIVE_ENDPOINT")
                .unwrap_or_else(|_| "https://www.googleapis.com".to_string());
            let mut url_str = format!("{}/drive/v3/files/{}?fields=size,name", endpoint, file_id);
            let mut auth_header: Option<String> = None;
            if let Some(ref provider) = self.options.credential_provider {
                if let Some(creds) = provider
                    .get_credentials("drive.google.com")
                    .or_else(|| provider.get_credentials("googleapis.com"))
                {
                    if let Some(ref token) = creds.password {
                        if creds.login.as_deref() == Some("apikey") {
                            url_str.push_str(&format!("&key={}", token));
                        } else {
                            auth_header = Some(format!("Bearer {}", token));
                        }
                    }
                }
            }
            let mut request = self.client.get(&url_str);
            if let Some(ref auth) = auth_header {
                request = request.header("Authorization", auth);
            }
            let response = request
                .send()
                .await
                .map_err(|e| Error::Worker(format!("GDrive metadata request failed: {}", e)))?;
            if !response.status().is_success() {
                return Err(Error::Protocol(format!(
                    "GDrive error status: {}",
                    response.status()
                )));
            }
            #[derive(serde::Deserialize)]
            struct GDriveMetadata {
                name: String,
                size: Option<String>,
            }
            let meta: GDriveMetadata = response
                .json()
                .await
                .map_err(|e| Error::Protocol(format!("Failed to parse GDrive metadata: {}", e)))?;
            let total_length = match meta.size {
                Some(ref s) => s.parse::<u64>().ok(),
                None => None,
            };
            Ok(Metadata {
                final_uri: self.options.uri.clone(),
                total_length,
                name: Some(meta.name),
                range_supported: true,
                padding_ranges: Vec::new(),
                etag: None,
                last_modified: None,
            })
        }
        #[cfg(not(feature = "gdrive"))]
        {
            Err(Error::Protocol(
                "Google Drive feature not enabled".to_string(),
            ))
        }
    }
}

#[async_trait]
impl ProtocolWorker for GDriveWorker {
    async fn fetch_segment(
        &self,
        task_id: TaskId,
        segment: Segment,
        progress: Option<ProgressSender>,
        storage_client: Option<Arc<dyn crate::storage::StorageDispatch>>,
        throttler: Arc<Throttler>,
    ) -> Result<PieceData> {
        #[cfg(feature = "gdrive")]
        {
            let is_onedrive = self.options.uri.starts_with("onedrive://")
                || self.options.uri.contains("onedrive.live.com")
                || self.options.uri.contains("sharepoint.com");

            let response = if is_onedrive {
                let item_id = extract_onedrive_id(&self.options.uri)
                    .ok_or_else(|| Error::Protocol("Invalid OneDrive URI".to_string()))?;
                let endpoint = std::env::var("AURA_ONEDRIVE_ENDPOINT")
                    .unwrap_or_else(|_| "https://graph.microsoft.com".to_string());
                let url_str = format!("{}/v1.0/me/drive/items/{}/content", endpoint, item_id);
                let mut request = self.client.get(&url_str);
                if let Some(ref provider) = self.options.credential_provider {
                    if let Some(creds) = provider
                        .get_credentials("graph.microsoft.com")
                        .or_else(|| provider.get_credentials("onedrive.com"))
                    {
                        if let Some(ref token) = creds.password {
                            request = request.header("Authorization", format!("Bearer {}", token));
                        }
                    }
                }
                if segment.length != u64::MAX {
                    request = request.header(
                        "Range",
                        format!(
                            "bytes={}-{}",
                            segment.offset,
                            segment.offset + segment.length - 1
                        ),
                    );
                } else if segment.offset > 0 {
                    request = request.header("Range", format!("bytes={}-", segment.offset));
                }
                request
                    .send()
                    .await
                    .map_err(|e| Error::Worker(format!("OneDrive request failed: {}", e)))?
            } else {
                let file_id = extract_file_id(&self.options.uri)
                    .ok_or_else(|| Error::Protocol("Invalid Google Drive URI".to_string()))?;
                let endpoint = std::env::var("AURA_GDRIVE_ENDPOINT")
                    .unwrap_or_else(|_| "https://www.googleapis.com".to_string());
                let mut url_str = format!("{}/drive/v3/files/{}?alt=media", endpoint, file_id);
                let mut auth_header: Option<String> = None;
                if let Some(ref provider) = self.options.credential_provider {
                    if let Some(creds) = provider
                        .get_credentials("drive.google.com")
                        .or_else(|| provider.get_credentials("googleapis.com"))
                    {
                        if let Some(ref token) = creds.password {
                            if creds.login.as_deref() == Some("apikey") {
                                url_str.push_str(&format!("&key={}", token));
                            } else {
                                auth_header = Some(format!("Bearer {}", token));
                            }
                        }
                    }
                }
                let mut request = self.client.get(&url_str);
                if let Some(ref auth) = auth_header {
                    request = request.header("Authorization", auth);
                }
                if segment.length != u64::MAX {
                    request = request.header(
                        "Range",
                        format!(
                            "bytes={}-{}",
                            segment.offset,
                            segment.offset + segment.length - 1
                        ),
                    );
                } else if segment.offset > 0 {
                    request = request.header("Range", format!("bytes={}-", segment.offset));
                }
                request
                    .send()
                    .await
                    .map_err(|e| Error::Worker(format!("GDrive request failed: {}", e)))?
            };

            let status = response.status();
            let sent_range = segment.length != u64::MAX || segment.offset > 0;
            if sent_range && status == reqwest::StatusCode::OK && segment.offset > 0 {
                return Err(Error::Protocol(format!(
                    "Server returned 200 OK for a ranged request at offset {}. Range header was not honoured.",
                    segment.offset
                )));
            }
            if !status.is_success() {
                return Err(Error::Protocol(format!(
                    "Cloud Drive error status: {}",
                    status
                )));
            }

            let mut stream = response.bytes_stream();
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
            let buffer_cap = self.options.http_buffer_capacity;
            let mut buffer = bytes::BytesMut::with_capacity(buffer_cap);
            let mut bytes_downloaded = 0u64;

            while let Some(chunk_res) = stream.next().await {
                let chunk = chunk_res
                    .map_err(|e| Error::Protocol(format!("Cloud Drive stream error: {}", e)))?;
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
        #[cfg(not(feature = "gdrive"))]
        {
            let _ = task_id;
            let _ = segment;
            let _ = progress;
            let _ = storage_client;
            let _ = throttler;
            Err(Error::Protocol(
                "Google Drive feature not enabled".to_string(),
            ))
        }
    }

    fn available_capacity(&self) -> usize {
        1
    }
}
