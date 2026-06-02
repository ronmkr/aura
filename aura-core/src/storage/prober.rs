use std::path::Path;
use std::time::{Duration, Instant};
use tokio::fs::OpenOptions;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum AllocationMethod {
    Fallocate,
    Sparse,
    ZeroFill,
}

pub struct AllocationProber;

impl AllocationProber {
    pub async fn probe<P: AsRef<Path>>(dir: P) -> std::io::Result<(AllocationMethod, Duration)> {
        let mut best_method = AllocationMethod::Sparse;
        let mut best_dur = Duration::MAX;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let test_size = 10 * 1024 * 1024; // 10 MB

        // 1. Test Sparse
        {
            let test_file = dir.as_ref().join(format!(".aura_probe_sparse_{}", ts));
            let start = Instant::now();
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&test_file)
                .await?;
            file.set_len(test_size).await?;
            file.sync_all().await?;
            let dur = start.elapsed();
            if dur < best_dur {
                best_dur = dur;
                best_method = AllocationMethod::Sparse;
            }
            let _ = tokio::fs::remove_file(&test_file).await;
        }

        // 2. Test ZeroFill
        {
            let test_file = dir.as_ref().join(format!(".aura_probe_zero_{}", ts));
            let start = Instant::now();
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&test_file)
                .await?;
            use tokio::io::AsyncWriteExt;
            let zeros = vec![0u8; 1024 * 1024];
            for _ in 0..10 {
                file.write_all(&zeros).await?;
            }
            file.sync_all().await?;
            let dur = start.elapsed();
            if dur < best_dur {
                best_dur = dur;
                best_method = AllocationMethod::ZeroFill;
            }
            let _ = tokio::fs::remove_file(&test_file).await;
        }

        // 3. Test Fallocate
        {
            let test_file = dir.as_ref().join(format!(".aura_probe_fallocate_{}", ts));
            let start = Instant::now();
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&test_file)
                .await?;

            let std_file = file.try_clone().await?.into_std().await;
            let _ = tokio::task::spawn_blocking(move || {
                let _ = crate::storage::sys::harden_file(&std_file, test_size);
            })
            .await;

            file.sync_all().await?;
            let dur = start.elapsed();
            if dur < best_dur {
                best_dur = dur;
                best_method = AllocationMethod::Fallocate;
            }
            let _ = tokio::fs::remove_file(&test_file).await;
        }

        Ok((best_method, best_dur))
    }
}

#[cfg(test)]
#[path = "prober_tests.rs"]
mod tests;
