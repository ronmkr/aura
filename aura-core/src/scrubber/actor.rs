use crate::{Result, TaskId};
use sha2::digest::Digest as _;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Debug)]
pub enum ScrubberCommand {
    ScrubNonSwarm {
        task_id: TaskId,
        path: PathBuf,
        checksum: crate::Checksum,
    },
    ScrubSwarm {
        task_id: TaskId,
        path: PathBuf,
        bt_task: Arc<crate::worker::bittorrent::task::BtTask>,
    },
}

#[derive(Debug)]
pub enum ScrubberEvent {
    PieceCorrupted(TaskId, usize),
    ScrubComplete(TaskId),
    ScrubFailed(TaskId, String),
}

pub struct IntegrityScrubber {
    command_rx: mpsc::Receiver<ScrubberCommand>,
    event_tx: mpsc::Sender<ScrubberEvent>,
}

impl IntegrityScrubber {
    pub fn new(
        command_rx: mpsc::Receiver<ScrubberCommand>,
        event_tx: mpsc::Sender<ScrubberEvent>,
    ) -> Self {
        Self {
            command_rx,
            event_tx,
        }
    }

    pub async fn run(mut self) {
        info!("Integrity Scrubber Actor started");
        while let Some(cmd) = self.command_rx.recv().await {
            match cmd {
                ScrubberCommand::ScrubNonSwarm {
                    task_id,
                    path,
                    checksum,
                } => {
                    info!(%task_id, "Scrubbing non-swarm task");
                    if let Err(e) = self.scrub_non_swarm(task_id, path, checksum).await {
                        error!(%task_id, error = %e, "Failed to scrub non-swarm task");
                        let _ = self
                            .event_tx
                            .send(ScrubberEvent::ScrubFailed(task_id, e.to_string()))
                            .await;
                    }
                }
                ScrubberCommand::ScrubSwarm {
                    task_id,
                    path,
                    bt_task,
                } => {
                    info!(%task_id, "Scrubbing swarm task");
                    if let Err(e) = self.scrub_swarm(task_id, path, bt_task).await {
                        error!(%task_id, error = %e, "Failed to scrub swarm task");
                        let _ = self
                            .event_tx
                            .send(ScrubberEvent::ScrubFailed(task_id, e.to_string()))
                            .await;
                    }
                }
            }
        }
    }

    async fn scrub_non_swarm(
        &mut self,
        task_id: TaskId,
        base_path: PathBuf,
        checksum: crate::Checksum,
    ) -> Result<()> {
        let part_path = crate::storage::ops::get_part_path(&base_path)?;
        let file = tokio::fs::File::open(&part_path).await?;
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
            warn!(%task_id, "Scrubber found mismatch in non-swarm task");
            // For non-swarm, the whole file is corrupt (piece 0)
            let _ = self
                .event_tx
                .send(ScrubberEvent::PieceCorrupted(task_id, 0))
                .await;
        }

        let _ = self
            .event_tx
            .send(ScrubberEvent::ScrubComplete(task_id))
            .await;
        Ok(())
    }

    async fn scrub_swarm(
        &mut self,
        task_id: TaskId,
        base_path: PathBuf,
        bt_task: Arc<crate::worker::bittorrent::task::BtTask>,
    ) -> Result<()> {
        let part_path = crate::storage::ops::get_part_path(&base_path)?;
        let mut file = tokio::fs::File::open(&part_path).await?;

        use sha1::Digest;
        use tokio::io::{AsyncReadExt, AsyncSeekExt};

        let torrent_opt = bt_task.state.torrent.lock().await;
        let torrent = match torrent_opt.as_ref() {
            Some(t) => t,
            None => return Ok(()),
        };
        let pieces = match &torrent.info.pieces {
            Some(p) => p,
            None => {
                warn!(%task_id, "Scrubber cannot verify v2 only torrents yet");
                return Ok(());
            }
        };
        let num_pieces = pieces.len() / 20;
        let piece_length = torrent.info.piece_length as u64;

        // Verify each piece that the bitfield marks as 'completed'
        let bitfield_opt = bt_task.state.bitfield.lock().await.clone();
        let bitfield = match bitfield_opt {
            Some(bf) => bf,
            None => return Ok(()),
        };
        for piece_index in 0..num_pieces {
            if !bitfield.get(piece_index) {
                continue;
            }

            let expected_hash = &pieces[piece_index * 20..(piece_index + 1) * 20];
            let offset = (piece_index as u64) * piece_length;

            // The last piece might be shorter
            let file_len = file.metadata().await?.len();
            if offset >= file_len {
                warn!(%task_id, piece_index, "Scrubber found EOF before piece");
                let _ = self
                    .event_tx
                    .send(ScrubberEvent::PieceCorrupted(task_id, piece_index))
                    .await;
                continue;
            }

            let mut len = piece_length;
            if offset + len > file_len {
                len = file_len - offset;
            }

            file.seek(std::io::SeekFrom::Start(offset)).await?;
            let mut buffer = vec![0u8; len as usize];
            file.read_exact(&mut buffer).await?;

            let mut hasher = sha1::Sha1::new();
            hasher.update(&buffer);
            let actual_hash = hasher.finalize();

            if actual_hash.as_slice() != expected_hash {
                warn!(%task_id, piece_index, "Scrubber found corrupted piece in swarm task");
                let _ = self
                    .event_tx
                    .send(ScrubberEvent::PieceCorrupted(task_id, piece_index))
                    .await;
            }
        }

        let _ = self
            .event_tx
            .send(ScrubberEvent::ScrubComplete(task_id))
            .await;
        Ok(())
    }
}
