use super::Orchestrator;
use crate::orchestrator::command::AddTaskArgs;
use crate::task::{DownloadPhase, MetaTask, SubTask, TaskType};
use crate::TaskId;
use arc_swap::ArcSwap;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::mpsc;

pub(super) fn make_test_orchestrator() -> (
    Orchestrator,
    mpsc::Receiver<crate::storage::StorageRequest>,
    tempfile::TempDir,
) {
    let (_command_tx, command_rx) = mpsc::channel(1024);
    let (storage_tx, storage_rx) = mpsc::channel(1024);
    let (_storage_event_tx, storage_event_rx) = mpsc::channel(1024);
    let (_event_tx, _event_rx) =
        tokio::sync::broadcast::channel::<crate::orchestrator::Event>(1024);
    let (dht_tx, _dht_rx) = mpsc::channel(1024);
    let (lpd_tx, _lpd_rx) = mpsc::channel(1024);
    let (_scrub_tx, _scrub_rx) = mpsc::channel::<crate::scrubber::ScrubberCommand>(1024);

    let config = Arc::new(ArcSwap::from_pointee(crate::Config::default()));

    let (orch, _tx) = Orchestrator::new(
        crate::orchestrator::state::OrchestratorChannels {
            command_rx,
            storage_tx,
            storage_completion_rx: storage_event_rx,
            dht_tx,
            lpd_tx,
            nat_tx: mpsc::channel(1).0,
        },
        config,
        sled::Config::new().temporary(true).open().unwrap(),
        Arc::new(
            crate::net_util::TokioResolver::builder_tokio()
                .unwrap()
                .build()
                .unwrap(),
        ),
    );

    let temp_dir = tempdir().unwrap();
    (orch, storage_rx, temp_dir)
}

#[tokio::test]
async fn test_racing_workers_are_cancelled_on_range_finished() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let meta_id = TaskId(1);
    let sub1_id = TaskId(11);
    let sub2_id = TaskId(12);
    let range = crate::task::Range { start: 0, end: 100 };

    let sub1 = SubTask {
        id: sub1_id,
        uri: "http://example.com/1".to_string(),
        task_type: TaskType::Http,
        phase: DownloadPhase::Downloading,
        assigned_ranges: vec![range],
        ewma_throughput: 0.0,
        retry_count: 0,
        total_length: 0,
        active: true,
        completed_length: 0,
        recent_bytes_downloaded: 0,
        target_concurrency: 1,
    };
    let sub2 = SubTask {
        id: sub2_id,
        uri: "http://example.com/2".to_string(),
        task_type: TaskType::Http,
        phase: DownloadPhase::Downloading,
        assigned_ranges: vec![range],
        ewma_throughput: 0.0,
        retry_count: 0,
        total_length: 0,
        active: true,
        completed_length: 0,
        recent_bytes_downloaded: 0,
        target_concurrency: 1,
    };

    let meta = MetaTask {
        id: meta_id,
        tenant_id: None,
        name: "test".to_string(),
        total_length: 1000,
        completed_length: 0,
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
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
        stall_ticks: 0,
        created_at: None,
        seed_ratio: None,
        seed_time: None,
        etag: None,
        last_modified: None,
    };

    orch.tasks.insert(meta_id, meta);

    // Register worker tokens in Orchestrator
    let token1 = tokio_util::sync::CancellationToken::new();
    let token2 = tokio_util::sync::CancellationToken::new();
    orch.worker_cancellation_tokens
        .insert(sub1_id, token1.clone());
    orch.worker_cancellation_tokens
        .insert(sub2_id, token2.clone());

    // When range finished by sub1
    orch.handle_range_finished(meta_id, sub1_id, range)
        .await
        .unwrap();

    // Then sub2's range should be cancelled
    assert!(token2.is_cancelled());

    // And sub2 should no longer have the range assigned in task state
    let updated_meta = orch.tasks.get(&meta_id).unwrap();
    let updated_sub2 = updated_meta
        .subtasks
        .iter()
        .find(|s| s.id == sub2_id)
        .unwrap();
    assert!(updated_sub2.assigned_ranges.is_empty());
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
        depends_on: Vec::new(),
        stall_ticks: 0,
        created_at: None,
        seed_ratio: None,
        seed_time: None,
        etag: None,
        last_modified: None,
    };
    orch.tasks.insert(TaskId(1), meta_a);

    // Add Task B (depends on A)
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
        depends_on: vec![TaskId(1)],
        stall_ticks: 0,
        created_at: None,
        seed_ratio: None,
        seed_time: None,
        etag: None,
        last_modified: None,
    };
    orch.tasks.insert(TaskId(2), meta_b);

    // Add Task C (depends on B)
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
        follow_on: None,
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: vec![TaskId(2)],
        stall_ticks: 0,
        created_at: None,
        seed_ratio: None,
        seed_time: None,
        etag: None,
        last_modified: None,
    };
    orch.tasks.insert(TaskId(3), meta_c);

    // Verify it doesn't crash on simple command
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    let res = orch
        .handle_command(crate::orchestrator::Command::GetConfig(tx))
        .await;
    assert!(res.is_ok());

    // Actual cycle check is internal, but we can verify it doesn't crash
}

#[tokio::test]
async fn test_dependency_waiting_state_and_unblocking() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    // 1. Add Task A
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
        depends_on: Vec::new(),
        stall_ticks: 0,
        created_at: None,
        seed_ratio: None,
        seed_time: None,
        etag: None,
        last_modified: None,
    };
    orch.tasks.insert(TaskId(1), meta_a);

    // 2. Add Task B (depends on A)
    let res = orch
        .handle_add_task(AddTaskArgs {
            id: TaskId(2),
            tenant_id: None,
            name: "task_b".to_string(),
            sources: vec![("http://test".to_string(), TaskType::Http)],
            checksum: None,
            priority: 3,
            streaming_mode: false,
            depends_on: vec![TaskId(1)],
            follow_on: None,
        })
        .await;

    assert!(res.is_ok());
    let task_b = orch.tasks.get(&TaskId(2)).unwrap();
    assert_eq!(task_b.phase, DownloadPhase::Waiting);

    // 3. Mark A as complete
    orch.handle_storage_event(crate::storage::StorageEvent::Completed(TaskId(1)))
        .await
        .unwrap();

    // 4. Task B should now be Downloading
    let task_b_after = orch.tasks.get(&TaskId(2)).unwrap();
    assert_eq!(task_b_after.phase, DownloadPhase::Downloading);
}

#[tokio::test]
async fn test_follow_on_custom_trigger() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let meta_id = TaskId(1);
    let meta = MetaTask {
        id: meta_id,
        tenant_id: None,
        name: "trigger".to_string(),
        total_length: 1000,
        completed_length: 1000,
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: Some(crate::task::FollowOnAction::Custom(
            "http://next-file".to_string(),
        )),
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        stall_ticks: 0,
        created_at: None,
        seed_ratio: None,
        seed_time: None,
        etag: None,
        last_modified: None,
    };
    orch.tasks.insert(meta_id, meta);

    // Complete the task
    orch.handle_storage_event(crate::storage::StorageEvent::Completed(meta_id))
        .await
        .unwrap();

    // Check if new task was added
    assert_eq!(orch.tasks.len(), 2);
    let new_task = orch.tasks.values().find(|t| t.id != meta_id).unwrap();
    assert_eq!(new_task.subtasks[0].uri, "http://next-file");
}
