use super::{StorageEngine, StorageEvent, StorageRequest};
use crate::worker::Segment;
use crate::TaskId;
use bytes::Bytes;
use tempfile::tempdir;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_storage_engine_atomic_completion() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("final_file.bin");
    let (request_tx, request_rx) = mpsc::channel(1);
    let (completion_tx, _completion_rx) = mpsc::channel(1);
    let mut storage = StorageEngine::new(request_rx, completion_tx, None, None);
    let id = TaskId(100);
    storage
        .register_task(id, file_path.clone(), 0, None, Vec::new())
        .await;

    tokio::spawn(async move {
        storage.run().await.unwrap();
    });

    let data = Bytes::from("atomic data");
    request_tx
        .send(StorageRequest::Write {
            task_id: id,
            segment: Segment {
                offset: 0,
                length: data.len() as u64,
            },
            data: data.into(),
            guard: None,
            generation: None,
        })
        .await
        .unwrap();

    request_tx.send(StorageRequest::Complete(id)).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    assert!(file_path.exists());
    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "atomic data");
}

#[tokio::test]
async fn test_storage_engine_sequential_aggregation() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("seq_file.bin");

    let (request_tx, request_rx) = mpsc::channel(10);
    let (completion_tx, _completion_rx) = mpsc::channel(1);
    let mut storage = StorageEngine::new(request_rx, completion_tx, None, None);
    let id = TaskId(200);
    storage
        .register_task(id, file_path.clone(), 0, None, Vec::new())
        .await;

    tokio::spawn(async move {
        storage.run().await.unwrap();
    });

    request_tx
        .send(StorageRequest::Write {
            task_id: id,
            segment: Segment {
                offset: 10,
                length: 5,
            },
            data: Bytes::from("world").into(),
            guard: None,
            generation: None,
        })
        .await
        .unwrap();

    request_tx
        .send(StorageRequest::Write {
            task_id: id,
            segment: Segment {
                offset: 0,
                length: 10,
            },
            data: Bytes::from("hello_seq_").into(),
            guard: None,
            generation: None,
        })
        .await
        .unwrap();

    request_tx.send(StorageRequest::Complete(id)).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    assert!(file_path.exists());
    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "hello_seq_world");
}

#[tokio::test]
async fn test_storage_engine_fsync_durability() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("durable_file.bin");
    let (request_tx, request_rx) = mpsc::channel(1);
    let (completion_tx, mut completion_rx) = mpsc::channel(1);
    let mut storage = StorageEngine::new(request_rx, completion_tx, None, None);
    let id = TaskId(300);
    storage
        .register_task(id, file_path.clone(), 0, None, Vec::new())
        .await;

    tokio::spawn(async move {
        storage.run().await.unwrap();
    });

    let data = Bytes::from("durable data");
    request_tx
        .send(StorageRequest::Write {
            task_id: id,
            segment: Segment {
                offset: 0,
                length: data.len() as u64,
            },
            data: data.into(),
            guard: None,
            generation: None,
        })
        .await
        .unwrap();

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
    assert_eq!(content, "durable data");
}
