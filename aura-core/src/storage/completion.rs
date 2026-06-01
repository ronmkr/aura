use super::utils::{get_part_path, sync_parent_dir};
use super::StorageEngine;
use crate::{Error, Result, TaskId};
use tokio::fs::{self, File};
use tracing::{error, info};

impl StorageEngine {
    pub(crate) async fn handle_complete(&mut self, id: TaskId) -> Result<()> {
        self.flush_all_pending(id).await?;

        let tasks = self.scheduler.extract_all_for_task(id);
        for task in tasks {
            self.execute_io_task(task).await?;
        }

        // Close the write handle before verification to ensure all data is committed
        // and to allow clean read-only access.
        self.handles.pop(&id);

        // Perform integrity verification if a checksum was provided
        if let Err(e) = self.verify_checksum(id).await {
            error!(%id, error = %e, "Integrity verification failed");
            return Err(e);
        }

        let base_path = self.task_paths.get(&id).ok_or(Error::TaskNotFound(id))?;
        let part_path = get_part_path(base_path)?;
        let hardened_base = crate::storage::sys::harden_path(base_path);

        // fsync the .part file to ensure all data is on disk before exposing it under the final name
        let file = fs::OpenOptions::new().read(true).open(&part_path).await?;
        file.sync_all().await?;

        info!(%id, from = ?part_path, to = ?hardened_base, "Performing atomic completion rename");
        fs::rename(&part_path, &hardened_base).await?;

        // Sync parent directory to ensure metadata rename is durable on Unix
        sync_parent_dir(&hardened_base).await;

        let _ = self
            .completion_tx
            .send(crate::storage::StorageEvent::Completed(id))
            .await;

        Ok(())
    }

    pub(crate) async fn verify_checksum(&mut self, id: TaskId) -> Result<()> {
        let checksum = match self.task_checksums.get(&id) {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        info!(%id, ?checksum, "Verifying file integrity");

        let base_path = self.task_paths.get(&id).ok_or(Error::TaskNotFound(id))?;
        let part_path = get_part_path(base_path)?;

        let file = File::open(&part_path).await?;
        let mut reader = tokio::io::BufReader::new(file);

        use md5::Digest;
        use tokio::io::AsyncReadExt;

        let actual = match checksum {
            crate::Checksum::Md5(ref expected) => {
                let mut hasher = md5::Md5::default();
                let mut buffer = [0u8; 65536];
                loop {
                    let n = reader.read(&mut buffer).await?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
            crate::Checksum::Sha1(ref expected) => {
                let mut hasher = sha1::Sha1::default();
                let mut buffer = [0u8; 65536];
                loop {
                    let n = reader.read(&mut buffer).await?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
            crate::Checksum::Sha256(ref expected) => {
                let mut hasher = sha2::Sha256::default();
                let mut buffer = [0u8; 65536];
                loop {
                    let n = reader.read(&mut buffer).await?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
            crate::Checksum::Sha512(ref expected) => {
                let mut hasher = sha2::Sha512::default();
                let mut buffer = [0u8; 65536];
                loop {
                    let n = reader.read(&mut buffer).await?;
                    if n == 0 {
                        break;
                    }
                    hasher.update(&buffer[..n]);
                }
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
        };

        let (expected, actual_hash) = actual;

        if expected.to_lowercase() != actual_hash.to_lowercase() {
            return Err(Error::Storage(format!(
                "Checksum mismatch: expected {}, got {}",
                expected, actual_hash
            )));
        }

        info!(%id, "Integrity verification successful");
        Ok(())
    }
}
