use super::*;
use crate::task::{DownloadPhase, MetaTask, Range, SubTask, TaskType};
use crate::TaskId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn make_test_orchestrator() -> (
    Orchestrator,
    mpsc::Receiver<crate::storage::StorageRequest>,
    tempfile::TempDir,
) {
    let (_command_tx, command_rx) = mpsc::channel(100);
    let (storage_tx, storage_rx) = mpsc::channel(100);
    let (_completion_tx, completion_rx) = mpsc::channel(100);
    let (dht_tx, _) = mpsc::channel(100);
    let (lpd_tx, _) = mpsc::channel(100);
    let (nat_tx, _) = mpsc::channel(100);

    let config = crate::Config::default();
    let config_swap = Arc::new(arc_swap::ArcSwap::from_pointee(config));
    let temp_dir = tempfile::tempdir().unwrap();
    let db = sled::open(temp_dir.path()).unwrap();
    let dns_resolver = Arc::new(
        hickory_resolver::TokioResolver::builder_tokio()
            .unwrap()
            .build()
            .unwrap(),
    );

    let (orchestrator, _event_tx) = Orchestrator::new(
        command_rx,
        storage_tx,
        completion_rx,
        dht_tx,
        lpd_tx,
        nat_tx,
        config_swap,
        db,
        dns_resolver,
    );

    (orchestrator, storage_rx, temp_dir)
}

#[tokio::test]
async fn test_racing_workers_are_cancelled_on_range_finished() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let meta_id = TaskId(1);
    let sub1_id = TaskId(101);
    let sub2_id = TaskId(102);
    let range = Range {
        start: 0,
        end: 1000,
    };

    let sub1 = SubTask {
        id: sub1_id,
        task_type: TaskType::Http,
        uri: "http://example.com/sub1".to_string(),
        assigned_ranges: vec![range],
        total_length: 1000,
        completed_length: 0,
        active: true,
        phase: DownloadPhase::Downloading,
        target_concurrency: 1,
        recent_bytes_downloaded: 0,
        ewma_throughput: 0.0,
        retry_count: 0,
    };

    let sub2 = SubTask {
        id: sub2_id,
        task_type: TaskType::Http,
        uri: "http://example.com/sub2".to_string(),
        assigned_ranges: vec![range],
        total_length: 1000,
        completed_length: 0,
        active: true,
        phase: DownloadPhase::Downloading,
        target_concurrency: 1,
        recent_bytes_downloaded: 0,
        ewma_throughput: 0.0,
        retry_count: 0,
    };

    let meta = MetaTask {
        id: meta_id,
        tenant_id: None,
        name: "test".to_string(),
        total_length: 1000,
        completed_length: 0,
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 100,
        streaming_mode: false,
        range_supported: true,
        subtasks: vec![sub1.clone(), sub2.clone()],
        pending_ranges: Vec::new(),
        in_flight_ranges: vec![(sub1_id, range), (sub2_id, range)],
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        follow_on: None,
    };

    orch.tasks.insert(meta_id, meta);

    // Register worker tokens in Orchestrator
    let token_sub1 = CancellationToken::new();
    let token_sub2 = CancellationToken::new();
    orch.worker_cancellation_tokens
        .insert(sub1_id, token_sub1.clone());
    orch.worker_cancellation_tokens
        .insert(sub2_id, token_sub2.clone());

    // Call handle_range_finished for sub1.
    // This finishes the range. Since sub2 was racing for the same range, it should be cancelled!
    let res = orch.handle_range_finished(meta_id, sub1_id, range).await;
    assert!(res.is_ok());

    // Verify that sub2's token was cancelled!
    assert!(
        token_sub2.is_cancelled(),
        "Racing worker sub2 should be cancelled"
    );
    // Verify that sub1's token is NOT cancelled (sub1 finished successfully)
    assert!(
        !token_sub1.is_cancelled(),
        "Finished worker sub1 should not be cancelled"
    );

    // Verify that sub2 was removed from worker_cancellation_tokens
    assert!(!orch.worker_cancellation_tokens.contains_key(&sub2_id));
}

#[tokio::test]
async fn test_dependency_cycle_detection() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    // Add Task A
    let meta_a = MetaTask {
        id: TaskId(1),
        tenant_id: None,
        name: "task_a".to_string(),
        total_length: 1000,
        completed_length: 0,
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: vec![TaskId(2)], // depends on B
    };
    orch.tasks.insert(TaskId(1), meta_a);

    // Add Task B
    let meta_b = MetaTask {
        id: TaskId(2),
        tenant_id: None,
        name: "task_b".to_string(),
        total_length: 1000,
        completed_length: 0,
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: vec![TaskId(1)], // depends on A -> Cycle!
    };
    orch.tasks.insert(TaskId(2), meta_b);

    assert!(orch.has_cycle());

    // Try handle_change_option introducing a cycle
    let meta_c = MetaTask {
        id: TaskId(3),
        tenant_id: None,
        name: "task_c".to_string(),
        total_length: 1000,
        completed_length: 0,
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        follow_on: None,
    };
    orch.tasks.insert(TaskId(3), meta_c);

    // Change option on C to depend on C -> Cycle!
    let res = orch
        .handle_change_option(TaskId(3), None, Some(vec![TaskId(3)]))
        .await;
    assert!(res.is_err());
    assert_eq!(
        res.unwrap_err().to_string(),
        "Engine error: Dependency cycle detected"
    );
}

#[tokio::test]
async fn test_dependency_waiting_state_and_unblocking() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    // 1. Add Task A (no deps)
    let meta_a = MetaTask {
        id: TaskId(1),
        tenant_id: None,
        name: "task_a".to_string(),
        total_length: 1000,
        completed_length: 0,
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        follow_on: None,
    };
    orch.tasks.insert(TaskId(1), meta_a);

    // 2. Add Task B (depends on A)
    let res = orch
        .handle_add_task(
            TaskId(2),
            None,
            "task_b".to_string(),
            Vec::new(),
            None,
            3,
            false,
            vec![TaskId(1)],
            None,
        )
        .await;
    assert!(res.is_ok());

    // B should be in Waiting state since A is Downloading (not Complete)
    let task_b = orch.tasks.get(&TaskId(2)).unwrap();
    assert_eq!(task_b.phase, DownloadPhase::Waiting);

    // 3. Mark Task A as Complete
    let storage_event = crate::storage::StorageEvent::Completed(TaskId(1));
    let res_storage = orch.handle_storage_event(storage_event).await;
    assert!(res_storage.is_ok());

    // A is now Complete
    let task_a = orch.tasks.get(&TaskId(1)).unwrap();
    assert_eq!(task_a.phase, DownloadPhase::Complete);

    // B should be unblocked and transition to Downloading!
    let task_b_new = orch.tasks.get(&TaskId(2)).unwrap();
    assert_eq!(task_b_new.phase, DownloadPhase::Downloading);
}

#[tokio::test]
async fn test_follow_on_custom_trigger() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let meta_id = TaskId(1);
    let follow_on_uri = "https://example.com/next_task".to_string();

    let meta = MetaTask {
        id: meta_id,
        tenant_id: None,
        name: "initial_task".to_string(),
        total_length: 1000,
        completed_length: 1000, // already finished
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        follow_on: Some(crate::task::FollowOnAction::Custom(follow_on_uri.clone())),
    };

    orch.tasks.insert(meta_id, meta);

    // Trigger storage completion event for Task 1
    let storage_event = crate::storage::StorageEvent::Completed(meta_id);
    let res = orch.handle_storage_event(storage_event).await;
    assert!(res.is_ok());

    // Verify that a new task was added (total tasks = 2)
    assert_eq!(orch.tasks.len(), 2);

    // Find the new task
    let new_task = orch
        .tasks
        .values()
        .find(|t| t.id != meta_id)
        .expect("A new task should have been added");

    assert_eq!(new_task.subtasks.len(), 1);
    assert_eq!(new_task.subtasks[0].uri, follow_on_uri);
}

#[path = "advanced_net_and_tenant_tests.rs"]
mod advanced_net_and_tenant_tests;
