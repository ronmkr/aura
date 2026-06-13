//! recheck: Background data verification and fast resume scan.

use crate::bitfield::Bitfield;
use crate::orchestrator::SubTaskEvent;
use crate::torrent::Torrent;
use crate::{Result, TaskId};
use sha1::Digest;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::sync::mpsc;
use tracing::{info, warn};

/// Runs the recheck loop for a BitTorrent task piece-by-piece.
pub async fn recheck_bittorrent(
    task_id: TaskId,
    sub_id: TaskId,
    part_path: &Path,
    torrent: Torrent,
    subtask_tx: mpsc::Sender<SubTaskEvent>,
    throttle_ms: u64,
) -> Result<()> {
    info!(%task_id, ?part_path, "Starting BitTorrent piece recheck");
    let num_pieces = torrent.pieces_count();
    let mut bitfield = Bitfield::new(num_pieces);

    let mut file = match File::open(part_path).await {
        Ok(f) => f,
        Err(e) => {
            warn!(%task_id, error = %e, "Could not open file for recheck; assuming clean state");
            let _ = subtask_tx
                .send(SubTaskEvent::RecheckComplete(task_id, bitfield))
                .await;
            return Ok(());
        }
    };

    let file_len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    if file_len == 0 {
        let _ = subtask_tx
            .send(SubTaskEvent::RecheckComplete(task_id, bitfield))
            .await;
        return Ok(());
    }

    let piece_len = torrent.info.piece_length;
    let mut read_buf = vec![0u8; 65536];

    for i in 0..num_pieces {
        let offset = i as u64 * piece_len;
        if offset >= file_len {
            continue;
        }

        if let Err(e) = file.seek(std::io::SeekFrom::Start(offset)).await {
            warn!(%task_id, piece = i, error = %e, "Seek failed during recheck");
            continue;
        }

        let mut bytes_to_read = std::cmp::min(piece_len, file_len - offset);
        let mut hasher = sha1::Sha1::new();
        let mut read_success = true;

        while bytes_to_read > 0 {
            let chunk_size = std::cmp::min(read_buf.len() as u64, bytes_to_read) as usize;
            match file.read_exact(&mut read_buf[..chunk_size]).await {
                Ok(_) => {
                    hasher.update(&read_buf[..chunk_size]);
                    bytes_to_read -= chunk_size as u64;
                }
                Err(e) => {
                    warn!(%task_id, piece = i, error = %e, "Read failed during recheck");
                    read_success = false;
                    break;
                }
            }

            if throttle_ms > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(throttle_ms)).await;
            }
        }

        if read_success {
            use sha1::Digest;
            let hash = hasher.finalize();
            if let Ok(expected) = torrent.piece_hash_v1(i) {
                if hash.as_slice() == expected {
                    bitfield.set(i, true);
                    let _ = subtask_tx
                        .send(SubTaskEvent::PieceVerified(task_id, sub_id, i))
                        .await;
                }
            }
        }

        let progress = (i + 1) as f64 / num_pieces as f64;
        let _ = subtask_tx
            .send(SubTaskEvent::RecheckProgress(task_id, progress))
            .await;
    }

    let _ = subtask_tx
        .send(SubTaskEvent::RecheckComplete(task_id, bitfield))
        .await;

    Ok(())
}

/// Runs the recheck validation for an HTTP/FTP task using file size or checksum.
pub async fn recheck_non_swarm(
    task_id: TaskId,
    _sub_id: TaskId,
    part_path: &Path,
    total_length: u64,
    checksum: Option<crate::Checksum>,
    subtask_tx: mpsc::Sender<SubTaskEvent>,
    throttle_ms: u64,
) -> Result<()> {
    info!(%task_id, ?part_path, "Starting HTTP/FTP recheck");
    let num_pieces = 128;
    let mut bitfield = Bitfield::new(num_pieces);

    let file = match File::open(part_path).await {
        Ok(f) => f,
        Err(e) => {
            warn!(%task_id, error = %e, "Could not open file for recheck; resuming from scratch");
            let _ = subtask_tx
                .send(SubTaskEvent::RecheckComplete(task_id, bitfield))
                .await;
            return Ok(());
        }
    };

    let file_len = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    if file_len == 0 || total_length == 0 {
        let _ = subtask_tx
            .send(SubTaskEvent::RecheckComplete(task_id, bitfield))
            .await;
        return Ok(());
    }

    // If checksum exists, verify full file
    let mut checksum_verified = false;
    if let Some(ref c) = checksum {
        let mut reader = tokio::io::BufReader::new(file);
        let mut buffer = [0u8; 65536];
        let mut verified = false;

        let actual = match c {
            crate::Checksum::Md5(ref expected) => {
                let mut hasher = md5::Md5::default();
                loop {
                    match reader.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(n) => {
                            hasher.update(&buffer[..n]);
                            if throttle_ms > 0 {
                                tokio::time::sleep(std::time::Duration::from_millis(throttle_ms))
                                    .await;
                            }
                        }
                        Err(_) => break,
                    }
                }
                use md5::Digest;
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
            crate::Checksum::Sha1(ref expected) => {
                let mut hasher = sha1::Sha1::default();
                loop {
                    match reader.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(n) => {
                            use sha1::Digest;
                            hasher.update(&buffer[..n]);
                            if throttle_ms > 0 {
                                tokio::time::sleep(std::time::Duration::from_millis(throttle_ms))
                                    .await;
                            }
                        }
                        Err(_) => break,
                    }
                }
                use sha1::Digest;
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
            crate::Checksum::Sha256(ref expected) => {
                let mut hasher = sha2::Sha256::default();
                loop {
                    match reader.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(n) => {
                            use sha2::Digest;
                            hasher.update(&buffer[..n]);
                            if throttle_ms > 0 {
                                tokio::time::sleep(std::time::Duration::from_millis(throttle_ms))
                                    .await;
                            }
                        }
                        Err(_) => break,
                    }
                }
                use sha2::Digest;
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
            crate::Checksum::Sha512(ref expected) => {
                let mut hasher = sha2::Sha512::default();
                loop {
                    match reader.read(&mut buffer).await {
                        Ok(0) => break,
                        Ok(n) => {
                            use sha2::Digest;
                            hasher.update(&buffer[..n]);
                            if throttle_ms > 0 {
                                tokio::time::sleep(std::time::Duration::from_millis(throttle_ms))
                                    .await;
                            }
                        }
                        Err(_) => break,
                    }
                }
                use sha2::Digest;
                let hash = hex::encode(hasher.finalize());
                (expected.clone(), hash)
            }
        };

        let (expected, actual_hash) = actual;
        if expected.to_lowercase() == actual_hash.to_lowercase() {
            verified = true;
        }

        if verified {
            checksum_verified = true;
            for idx in 0..num_pieces {
                bitfield.set(idx, true);
            }
            let _ = subtask_tx
                .send(SubTaskEvent::RecheckProgress(task_id, 1.0))
                .await;
        }
    }

    if !checksum_verified {
        // If checksum check failed or wasn't provided, resume from end of .part file
        let progress = (file_len as f64 / total_length as f64).clamp(0.0, 1.0);
        let completed_pieces = (progress * num_pieces as f64) as usize;

        for idx in 0..completed_pieces {
            bitfield.set(idx, true);
        }

        let _ = subtask_tx
            .send(SubTaskEvent::RecheckProgress(task_id, progress))
            .await;
    }

    let _ = subtask_tx
        .send(SubTaskEvent::RecheckComplete(task_id, bitfield))
        .await;

    Ok(())
}
