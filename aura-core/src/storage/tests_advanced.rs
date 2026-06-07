use super::{StorageEngine, StorageEvent, StorageRequest};
use crate::worker::Segment;
use crate::TaskId;
use bytes::Bytes;
use tempfile::tempdir;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_storage_engine_bit_bucket() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("bit_bucket.bin");
    let (request_tx, request_rx) = mpsc::channel(1);
    let (completion_tx, _completion_rx) = mpsc::channel(1);
    let mut storage = StorageEngine::new(request_rx, completion_tx, None, None);
    let id = TaskId(400);

    // Register task with a padding range from offset 10 to 20
    let padding_ranges = vec![crate::task::Range { start: 10, end: 20 }];
    storage
        .register_task(id, file_path.clone(), 30, None, padding_ranges)
        .await;

    tokio::spawn(async move {
        storage.run().await.unwrap();
    });

    // 1. Write to non-padding range (0-10)
    request_tx
        .send(StorageRequest::Write {
            task_id: id,
            segment: Segment {
                offset: 0,
                length: 10,
            },
            data: bytes::BytesMut::from(&b"1234567890"[..]),
            guard: None,
            generation: None,
        })
        .await
        .unwrap();

    // 2. Write to padding range (10-20) - SHOULD BE DISCARDED
    request_tx
        .send(StorageRequest::Write {
            task_id: id,
            segment: Segment {
                offset: 10,
                length: 10,
            },
            data: bytes::BytesMut::from(&b"PAD_DATA__"[..]),
            guard: None,
            generation: None,
        })
        .await
        .unwrap();

    // 3. Write to non-padding range (20-30)
    request_tx
        .send(StorageRequest::Write {
            task_id: id,
            segment: Segment {
                offset: 20,
                length: 10,
            },
            data: bytes::BytesMut::from(&b"ABCDEFGHIJ"[..]),
            guard: None,
            generation: None,
        })
        .await
        .unwrap();

    // Ensure all dirty buffers are flushed
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    request_tx.send(StorageRequest::Complete(id)).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    assert!(file_path.exists());
    let content = std::fs::read(&file_path).unwrap();
    assert_eq!(&content[0..10], b"1234567890");
    assert_eq!(&content[10..20], b"\0\0\0\0\0\0\0\0\0\0");
    assert_eq!(&content[20..30], b"ABCDEFGHIJ");
}

#[tokio::test]
async fn test_storage_engine_bit_bucket_complex() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("bit_bucket_complex.bin");
    let (request_tx, request_rx) = mpsc::channel(1);
    let (completion_tx, _completion_rx) = mpsc::channel(1);
    let mut storage = StorageEngine::new(request_rx, completion_tx, None, None);
    let id = TaskId(401);

    // Multiple padding ranges
    let padding_ranges = vec![
        crate::task::Range { start: 5, end: 10 },
        crate::task::Range { start: 15, end: 20 },
    ];
    storage
        .register_task(id, file_path.clone(), 25, None, padding_ranges)
        .await;

    tokio::spawn(async move {
        storage.run().await.unwrap();
    });

    // Write a large block covering data-pad-data-pad-data
    request_tx
        .send(StorageRequest::Write {
            task_id: id,
            segment: Segment {
                offset: 0,
                length: 25,
            },
            data: bytes::BytesMut::from(&b"12345PPPPP67890XXXXXabcde"[..]),
            guard: None,
            generation: None,
        })
        .await
        .unwrap();

    request_tx.send(StorageRequest::Complete(id)).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let content = std::fs::read(&file_path).unwrap();
    assert_eq!(&content[0..5], b"12345");
    assert_eq!(&content[5..10], b"\0\0\0\0\0");
    assert_eq!(&content[10..15], b"67890");
    assert_eq!(&content[15..20], b"\0\0\0\0\0");
    assert_eq!(&content[20..25], b"abcde");
}

#[tokio::test]
async fn test_storage_engine_generation_validation() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("gen_val_file.bin");
    let (request_tx, request_rx) = mpsc::channel(10);
    let (completion_tx, mut completion_rx) = mpsc::channel(10);
    let mut storage = StorageEngine::new(request_rx, completion_tx, None, None);
    let id = TaskId(999);
    storage
        .register_task(id, file_path.clone(), 0, None, Vec::new())
        .await;

    tokio::spawn(async move {
        storage.run().await.unwrap();
    });

    // Write 1: Generation 2
    let data1 = Bytes::from("newer data");
    request_tx
        .send(StorageRequest::Write {
            task_id: id,
            segment: Segment {
                offset: 0,
                length: data1.len() as u64,
            },
            data: data1.clone().into(),
            guard: None,
            generation: Some(2),
        })
        .await
        .unwrap();

    // Write 2: Generation 1 (stale, should be discarded)
    let data2 = Bytes::from("older junk");
    request_tx
        .send(StorageRequest::Write {
            task_id: id,
            segment: Segment {
                offset: 0,
                length: data2.len() as u64,
            },
            data: data2.clone().into(),
            guard: None,
            generation: Some(1),
        })
        .await
        .unwrap();

    // Send complete to flush and close
    request_tx.send(StorageRequest::Complete(id)).await.unwrap();

    let event = completion_rx.recv().await.unwrap();
    match event {
        StorageEvent::Completed(completed_id) => {
            assert_eq!(completed_id, id);
        }
        StorageEvent::Error(_, err) => {
            panic!("Completed with error: {}", err);
        }
    }

    assert!(file_path.exists());
    let content = std::fs::read_to_string(&file_path).unwrap();
    // Since the first write was gen 2 and the second was gen 1,
    // the gen 1 write should have been discarded and "newer data" should remain.
    assert_eq!(content, "newer data");
}
