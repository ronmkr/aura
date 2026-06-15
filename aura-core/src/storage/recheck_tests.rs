use super::recheck::{recheck_bittorrent, recheck_non_swarm};
use crate::orchestrator::SubTaskEvent;
use crate::torrent::{Info, Torrent};
use crate::{Checksum, TaskId};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;
use std::io::Write;
use tempfile::tempdir;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_recheck_bittorrent_success() {
    let dir = tempdir().unwrap();
    let part_path = dir.path().join("test_file.part");

    // Generate dummy piece data: 2 pieces of 1024 bytes each
    let piece0 = vec![0xAAu8; 1024];
    let piece1 = vec![0xBBu8; 1024];

    // Compute piece hashes
    let mut h0 = Sha1::new();
    h0.update(&piece0);
    let hash0 = h0.finalize();

    let mut h1 = Sha1::new();
    h1.update(&piece1);
    let hash1 = h1.finalize();

    let mut pieces = Vec::new();
    pieces.extend_from_slice(&hash0);
    pieces.extend_from_slice(&hash1);

    // Create Torrent metainfo
    let info = Info {
        name: "test_file".to_string(),
        piece_length: 1024,
        pieces: Some(pieces),
        length: Some(2048),
        files: None,
        meta_version: None,
        file_tree: None,
        private: None,
    };
    let torrent = Torrent {
        announce: "http://tracker.example.com/announce".to_string(),
        info,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        piece_layers: None,
    };

    // Write dummy file to disk
    {
        let mut file = std::fs::File::create(&part_path).unwrap();
        file.write_all(&piece0).unwrap();
        file.write_all(&piece1).unwrap();
    }

    let (tx, mut rx) = mpsc::channel(10);
    let task_id = TaskId(1);
    let sub_id = TaskId(2);

    recheck_bittorrent(task_id, sub_id, &part_path, torrent, tx, 0)
        .await
        .unwrap();

    let mut verified_pieces = Vec::new();
    let mut progresses = Vec::new();
    let mut completed_bf = None;

    while let Ok(event) = rx.try_recv() {
        match event {
            SubTaskEvent::PieceVerified(t_id, s_id, idx) => {
                assert_eq!(t_id, task_id);
                assert_eq!(s_id, sub_id);
                verified_pieces.push(idx);
            }
            SubTaskEvent::RecheckProgress(t_id, progress) => {
                assert_eq!(t_id, task_id);
                progresses.push(progress);
            }
            SubTaskEvent::RecheckComplete(t_id, bf) => {
                assert_eq!(t_id, task_id);
                completed_bf = Some(bf);
            }
            _ => panic!("Unexpected event"),
        }
    }

    assert_eq!(verified_pieces, vec![0, 1]);
    assert!(!progresses.is_empty());
    assert_eq!(*progresses.last().unwrap(), 1.0);

    let bf = completed_bf.unwrap();
    assert_eq!(bf.len(), 2);
    assert!(bf.get(0));
    assert!(bf.get(1));
}

#[tokio::test]
async fn test_recheck_non_swarm_checksum_match() {
    let dir = tempdir().unwrap();
    let part_path = dir.path().join("test_file.part");

    let content = b"hello world from aura engine";
    {
        let mut file = std::fs::File::create(&part_path).unwrap();
        file.write_all(content).unwrap();
    }

    let mut hasher = Sha256::new();
    hasher.update(content);
    let sha256_hex = hex::encode(hasher.finalize());

    let (tx, mut rx) = mpsc::channel(10);
    let task_id = TaskId(1);
    let sub_id = TaskId(2);

    recheck_non_swarm(
        task_id,
        sub_id,
        &part_path,
        content.len() as u64,
        Some(Checksum::Sha256(sha256_hex)),
        tx,
        0,
    )
    .await
    .unwrap();

    let mut completed_bf = None;
    while let Ok(event) = rx.try_recv() {
        match event {
            SubTaskEvent::RecheckComplete(t_id, bf) => {
                assert_eq!(t_id, task_id);
                completed_bf = Some(bf);
            }
            SubTaskEvent::RecheckProgress(t_id, progress) => {
                assert_eq!(t_id, task_id);
                assert_eq!(progress, 1.0);
            }
            _ => {}
        }
    }

    let bf = completed_bf.unwrap();
    assert_eq!(bf.len(), 128);
    assert_eq!(bf.count_set(), 128);
}

#[tokio::test]
async fn test_recheck_non_swarm_checksum_mismatch_resume() {
    let dir = tempdir().unwrap();
    let part_path = dir.path().join("test_file.part");

    let content = vec![0u8; 50];
    {
        let mut file = std::fs::File::create(&part_path).unwrap();
        file.write_all(&content).unwrap();
    }

    // Pass invalid checksum to trigger standard resume from end of file
    let (tx, mut rx) = mpsc::channel(10);
    let task_id = TaskId(1);
    let sub_id = TaskId(2);

    recheck_non_swarm(
        task_id,
        sub_id,
        &part_path,
        100, // Total length is 100, file length is 50 -> 50% done (64 pieces out of 128)
        Some(Checksum::Sha256("wrongchecksum".to_string())),
        tx,
        0,
    )
    .await
    .unwrap();

    let mut completed_bf = None;
    let mut final_progress = 0.0;
    while let Ok(event) = rx.try_recv() {
        match event {
            SubTaskEvent::RecheckComplete(t_id, bf) => {
                assert_eq!(t_id, task_id);
                completed_bf = Some(bf);
            }
            SubTaskEvent::RecheckProgress(t_id, progress) => {
                assert_eq!(t_id, task_id);
                final_progress = progress;
            }
            _ => {}
        }
    }

    assert_eq!(final_progress, 0.5);
    let bf = completed_bf.unwrap();
    assert_eq!(bf.len(), 128);
    assert_eq!(bf.count_set(), 64);
}
