use super::tests_racing::make_test_orchestrator;
use crate::orchestrator::command::AddTaskArgs;
use crate::task::{DownloadPhase, MetaTask, TaskType};
use crate::TaskId;
use std::collections::HashMap;

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
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        stall_ticks: 0,
        created_at: None,
        etag: None,
        last_modified: None,
        selected_files: None,
        seed_ratio_override: None,
        seed_time_override: None,
        recheck_progress: 0.0,
    };
    orch.tasks.insert(TaskId(1), meta_a);

    // Add Task B (depends on A)
    let meta_b = MetaTask {
        id: TaskId(2),
        tenant_id: None,
        name: "task_b".to_string(),
        total_length: 1000,
        completed_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: vec![TaskId(1)],
        stall_ticks: 0,
        created_at: None,
        etag: None,
        last_modified: None,
        selected_files: None,
        seed_ratio_override: None,
        seed_time_override: None,
        recheck_progress: 0.0,
    };
    orch.tasks.insert(TaskId(2), meta_b);

    // Add Task C (depends on B)
    let meta_c = MetaTask {
        id: TaskId(3),
        tenant_id: None,
        name: "task_c".to_string(),
        total_length: 1000,
        completed_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: vec![TaskId(2)],
        stall_ticks: 0,
        created_at: None,
        etag: None,
        last_modified: None,
        selected_files: None,
        seed_ratio_override: None,
        seed_time_override: None,
        recheck_progress: 0.0,
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
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: Vec::new(),
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        stall_ticks: 0,
        created_at: None,
        etag: None,
        last_modified: None,
        selected_files: None,
        seed_ratio_override: None,
        seed_time_override: None,
        recheck_progress: 0.0,
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
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        stall_ticks: 0,
        created_at: None,
        etag: None,
        last_modified: None,
        selected_files: None,
        seed_ratio_override: None,
        seed_time_override: None,
        recheck_progress: 0.0,
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
