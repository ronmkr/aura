use super::Orchestrator;
use crate::task::{DownloadPhase, MetaTask, SubTask, TaskType};
use crate::TaskId;
use std::collections::HashMap;
use tempfile::tempdir;
use tokio::sync::mpsc;

pub(super) fn make_test_orchestrator() -> (
    Orchestrator,
    mpsc::Receiver<crate::storage::StorageRequest>,
    tempfile::TempDir,
) {
    let (orch, storage_rx) = crate::orchestrator::test_helpers::create_test_orchestrator();
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
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        subtasks: vec![sub1.clone(), sub2.clone()],
        pending_ranges: Vec::new(),
        in_flight_ranges: vec![(sub1_id, range), (sub2_id, range)],
        checksum: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        follow_on: None,
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
