use crate::orchestrator::protocol_detector::{DetectedType, ProtocolDetector};
use crate::orchestrator::Engine;
use crate::{Result, TaskId, TenantId};
use std::path::{Path, PathBuf};
use tokio::fs;

impl Engine {
    /// Recursively scans a directory for `.torrent` and `.metalink` files and adds them as tasks.
    pub async fn add_from_folder(
        &self,
        tenant_id: Option<TenantId>,
        dir: &str,
        recursive: bool,
    ) -> Result<Vec<TaskId>> {
        let mut added_ids = Vec::new();
        let mut dirs_to_scan = vec![(PathBuf::from(dir), 0)];
        let max_depth = self.config.load().bulk.max_scan_depth; // Architectural safeguard

        while let Some((current_dir, depth)) = dirs_to_scan.pop() {
            let mut entries = match fs::read_dir(&current_dir).await {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!(dir = ?current_dir, error = %e, "Failed to read directory during bulk ingestion");
                    continue;
                }
            };

            while let Some(entry) = entries.next_entry().await.map_err(|e| {
                crate::Error::Engine(format!(
                    "Failed to read next entry in {:?}: {}",
                    current_dir, e
                ))
            })? {
                let path = entry.path();
                let metadata = entry.metadata().await.map_err(|e| {
                    crate::Error::Engine(format!("Failed to get metadata for {:?}: {}", path, e))
                })?;

                if metadata.is_dir() {
                    if recursive && depth < max_depth {
                        dirs_to_scan.push((path, depth + 1));
                    }
                } else {
                    let path_str = match path.to_str() {
                        Some(s) => s,
                        None => continue,
                    };

                    if let Some(detected) = ProtocolDetector::detect(path_str).await {
                        // For folder ingestion, we specifically look for container files
                        if matches!(detected, DetectedType::BitTorrent | DetectedType::Metalink) {
                            let name = path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or(crate::DEFAULT_TASK_NAME)
                                .to_string();

                            let id = TaskId::random();
                            match self
                                .add_task_with_sources(
                                    id,
                                    tenant_id.clone(),
                                    name,
                                    vec![(path_str.to_string(), detected.to_task_type())],
                                    None,
                                )
                                .await
                            {
                                Ok(handle) => added_ids.push(handle.id()),
                                Err(e) => {
                                    tracing::error!(path = %path_str, error = %e, "Failed to add task during bulk folder ingestion")
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(added_ids)
    }

    /// Reads a text file containing a list of URIs/hashes and adds them as tasks.
    pub async fn add_from_file(
        &self,
        tenant_id: Option<TenantId>,
        path: &str,
    ) -> Result<Vec<TaskId>> {
        let mut added_ids = Vec::new();
        let content = fs::read_to_string(path).await.map_err(|e| {
            crate::Error::Engine(format!(
                "Failed to read bulk ingestion file {}: {}",
                path, e
            ))
        })?;

        for (idx, line) in content.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(detected) = ProtocolDetector::detect(line).await {
                let name = format!(
                    "task-bulk-{}-{}",
                    Path::new(path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("file"),
                    idx
                );

                // Construct full AddTaskArgs to ensure tenant_id is preserved
                let id = TaskId::random();
                let args = crate::orchestrator::command::AddTaskArgs {
                    id,
                    tenant_id: tenant_id.clone(),
                    name,
                    sources: vec![(line.to_string(), detected.to_task_type())],
                    checksum: None,
                    priority: 3,
                    streaming_mode: false,
                    depends_on: Vec::new(),
                    follow_on: None,
                };

                match self.add_task_with_options(args).await {
                    Ok(handle) => added_ids.push(handle.id()),
                    Err(e) => {
                        tracing::error!(line = %line, error = %e, "Failed to add task during bulk file ingestion")
                    }
                }
            } else {
                tracing::warn!(line = %line, "Could not detect protocol for bulk ingestion line; skipping");
            }
        }

        Ok(added_ids)
    }
}
