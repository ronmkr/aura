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
        depends_on: vec![TaskId(2)], // depends on B
    };
    orch.tasks.insert(TaskId(1), meta_a);

    // Add Task B
    let meta_b = MetaTask {
        id: TaskId(2),
        name: "task_b".to_string(),
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
        depends_on: vec![TaskId(1)], // depends on A -> Cycle!
    };
    orch.tasks.insert(TaskId(2), meta_b);

    assert!(orch.has_cycle());

    // Try handle_change_option introducing a cycle
    let meta_c = MetaTask {
        id: TaskId(3),
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
    };
    orch.tasks.insert(TaskId(1), meta_a);

    // 2. Add Task B (depends on A)
    let res = orch
        .handle_add_task(
            TaskId(2),
            "task_b".to_string(),
            Vec::new(),
            None,
            3,
            false,
            vec![TaskId(1)],
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
async fn test_resource_preemption_logic() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    // Setup config: max concurrent downloads = 2, min connections per task = 2
    {
        let mut config = crate::Config::default();
        config.bandwidth.max_concurrent_downloads = 2;
        config.bandwidth.min_connections_per_task = 2;
        config.bandwidth.max_connections_per_task = 10;
        orch.config = Arc::new(arc_swap::ArcSwap::from_pointee(config));
    }

    // Add active Task A (prio 3, downloading)
    let meta_a = MetaTask {
        id: TaskId(1),
        name: "task_a".to_string(),
        total_length: 1000,
        completed_length: 0,
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        subtasks: vec![SubTask {
            id: TaskId(101),
            task_type: TaskType::Http,
            uri: "http://uri".to_string(),
            assigned_ranges: Vec::new(),
            total_length: 1000,
            completed_length: 0,
            active: true,
            phase: DownloadPhase::Downloading,
            target_concurrency: 8,
            recent_bytes_downloaded: 0,
            ewma_throughput: 0.0,
            retry_count: 0,
        }],
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
    };
    orch.tasks.insert(TaskId(1), meta_a);

    // Add active Task B (prio 4, downloading)
    let meta_b = MetaTask {
        id: TaskId(2),
        name: "task_b".to_string(),
        total_length: 1000,
        completed_length: 0,
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 4,
        streaming_mode: false,
        range_supported: true,
        subtasks: vec![SubTask {
            id: TaskId(102),
            task_type: TaskType::Http,
            uri: "http://uri".to_string(),
            assigned_ranges: Vec::new(),
            total_length: 1000,
            completed_length: 0,
            active: true,
            phase: DownloadPhase::Downloading,
            target_concurrency: 6,
            recent_bytes_downloaded: 0,
            ewma_throughput: 0.0,
            retry_count: 0,
        }],
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
    };
    orch.tasks.insert(TaskId(2), meta_b);

    // Register cancellation tokens
    orch.cancellation_tokens
        .insert(TaskId(1), CancellationToken::new());
    orch.cancellation_tokens
        .insert(TaskId(2), CancellationToken::new());

    // 3. Add High Priority (Prio 0) Task C (needs a slot!)
    // Since max_concurrent_downloads is 2, and we have A and B active,
    // B (the lowest priority active task with prio 4) should be preempted to Waiting!
    // A (prio 3) target concurrency should be scaled down to min (2)!
    let res = orch
        .handle_add_task(
            TaskId(3),
            "task_c".to_string(),
            Vec::new(),
            None,
            0,
            false,
            Vec::new(),
        )
        .await;
    assert!(res.is_ok());

    // Verify Task B was preempted to Waiting state
    let task_b = orch.tasks.get(&TaskId(2)).unwrap();
    assert_eq!(task_b.phase, DownloadPhase::Waiting);

    // Verify Task A was scaled down to min_connections_per_task (2)
    let task_a = orch.tasks.get(&TaskId(1)).unwrap();
    assert_eq!(task_a.subtasks[0].target_concurrency, 2);

    // Task C is Downloading
    let task_c = orch.tasks.get(&TaskId(3)).unwrap();
    assert_eq!(task_c.phase, DownloadPhase::Downloading);
}
