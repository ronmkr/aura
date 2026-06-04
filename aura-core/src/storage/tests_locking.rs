use super::*;
use crate::TaskId;
use tempfile::tempdir;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_storage_engine_file_locking() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("locked_file.bin");

    // Create channels for the storage engine
    let (request_tx, request_rx) = mpsc::channel(10);
    let (completion_tx, mut completion_rx) = mpsc::channel(10);
    let storage = StorageEngine::new(request_rx, completion_tx, None, None);

    tokio::spawn(async move {
        storage.run().await.unwrap();
    });

    let id1 = TaskId(701);
    let id2 = TaskId(702);

    // Register Task 1 with non-zero length to trigger pre-allocation and locking
    request_tx
        .send(StorageRequest::RegisterTask {
            task_id: id1,
            path: file_path.clone(),
            total_length: 100,
            checksum: None,
            padding_ranges: Vec::new(),
        })
        .await
        .unwrap();

    // Give some time for Task 1 registration to process and lock the file
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Register Task 2 using the same file. Pre-allocation should fail to lock and report an error.
    request_tx
        .send(StorageRequest::RegisterTask {
            task_id: id2,
            path: file_path.clone(),
            total_length: 100,
            checksum: None,
            padding_ranges: Vec::new(),
        })
        .await
        .unwrap();

    // We should receive an error event for Task 2
    let mut task2_failed = false;
    for _ in 0..5 {
        let event_opt =
            tokio::time::timeout(std::time::Duration::from_millis(500), completion_rx.recv()).await;
        if let Ok(Some(event)) = event_opt {
            match event {
                StorageEvent::Error(err_id, err_msg) => {
                    if err_id == id2 {
                        assert!(err_msg.contains("locked") || err_msg.contains("lock"));
                        task2_failed = true;
                        break;
                    }
                }
                StorageEvent::Completed(_) => {}
            }
        } else {
            break;
        }
    }
    assert!(task2_failed, "Task 2 did not fail with locking error");
}
