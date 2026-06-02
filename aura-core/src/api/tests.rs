use super::*;

use crate::task::TaskType;
use crate::Config;
use tokio_stream::StreamExt;

#[tokio::test]
async fn test_task_handle_events() {
    let config = Config::default();
    let (engine, _orchestrator, _storage) = Engine::new(config).await.unwrap();

    let event_tx = _orchestrator.event_tx.clone();
    let id = TaskId(123);
    let handle = TaskHandle::new(id, engine);
    let mut events = handle.events();

    // Emit events for our task
    event_tx
        .send(Event::MetadataResolved {
            id,
            final_uri: "http://example.com".to_string(),
            total_length: 1000,
            name: Some("test".to_string()),
        })
        .unwrap();

    event_tx
        .send(Event::TaskProgress {
            id,
            completed_bytes: 500,
            uploaded_bytes: 0,
            total_bytes: 1000,
        })
        .unwrap();

    // Emit event for ANOTHER task (should be filtered out)
    event_tx
        .send(Event::TaskProgress {
            id: TaskId(456),
            completed_bytes: 100,
            uploaded_bytes: 0,
            total_bytes: 1000,
        })
        .unwrap();

    event_tx.send(Event::TaskCompleted(id)).unwrap();

    // Verify we only received events for Task 123
    let e1 = events.next().await.unwrap();
    if let TaskEvent::MetadataResolved { total_length, .. } = e1 {
        assert_eq!(total_length, 1000);
    } else {
        panic!("Expected MetadataResolved");
    }

    let e2 = events.next().await.unwrap();
    if let TaskEvent::Progress {
        completed_bytes, ..
    } = e2
    {
        assert_eq!(completed_bytes, 500);
    } else {
        panic!("Expected Progress");
    }

    let e3 = events.next().await.unwrap();
    if let TaskEvent::Completed = e3 {
        // success
    } else {
        panic!("Expected Completed");
    }
}

#[test]
fn test_task_event_filtering_proptest() {
    // This is a placeholder for a more complex property-based test
    // validating that task-specific streams never leak data from other tasks.
}

#[tokio::test]
async fn test_engine_subscribe_captures_task_added() {
    let temp_dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.storage.download_dir = temp_dir.path().to_string_lossy().to_string();

    let (engine, orchestrator, storage) = Engine::new(config).await.unwrap();

    // Spawn actors
    tokio::spawn(async move {
        let _ = orchestrator.run().await;
    });
    tokio::spawn(async move {
        let _ = storage.run().await;
    });

    // 1. Subscribe BEFORE adding task
    let mut events = engine.subscribe();

    // 2. Add task
    let id = TaskId(999);
    engine
        .add_task_with_options(crate::orchestrator::command::AddTaskArgs {
            id,
            tenant_id: None,
            name: "test_race_task".to_string(),
            sources: vec![("http://example.com/file".to_string(), TaskType::Http)],
            checksum: None,
            priority: 100,
            streaming_mode: false,
            depends_on: Vec::new(),
            follow_on: None,
        })
        .await
        .unwrap();

    // 3. Verify Event::TaskAdded is captured
    let mut found = false;
    // Wait up to 2 seconds
    let timeout_fut = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while let Ok(event) = events.recv().await {
            if let Event::TaskAdded(task_id) = event {
                if task_id == id {
                    found = true;
                    break;
                }
            }
        }
    });
    let _ = timeout_fut.await;
    assert!(
        found,
        "Event::TaskAdded should be received by early subscriber"
    );
}
