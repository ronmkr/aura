use super::tests_racing::make_test_orchestrator;
use crate::task::{DownloadPhase, MetaTask, TaskType};
use crate::{TaskId, TenantId};
use std::collections::HashMap;

#[tokio::test]
async fn test_tenant_isolation_in_orchestrator() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();
    let tenant_id = TenantId("tenant1".to_string());

    // Register tenant with specific root
    orch.tenants.insert(
        tenant_id.clone(),
        crate::orchestrator::state::TenantContext {
            disk_path_root: Some(_temp_dir.path().to_path_buf()),
            throttler: orch.throttler.clone(),
            max_tasks: Some(10),
        },
    );

    let meta = MetaTask {
        id: TaskId(1),
        tenant_id: Some(tenant_id.clone()),
        name: "tenant_task".to_string(),
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
        created_at: Some(chrono::Utc::now()),
        etag: None,
        last_modified: None,
        selected_files: None,
        seed_ratio_override: None,
        seed_time_override: None,
        recheck_progress: 0.0,
    };

    orch.tasks.insert(TaskId(1), meta);

    let base_dir = orch.resolve_base_dir(&Some(tenant_id));
    assert_eq!(base_dir, _temp_dir.path());
}

#[tokio::test]
async fn test_adaptive_concurrency_scaling() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let meta_id = TaskId(1);
    let sub_id = TaskId(11);
    let sub = crate::task::SubTask {
        id: sub_id,
        uri: "http://slow".to_string(),
        task_type: TaskType::Http,
        phase: DownloadPhase::Downloading,
        assigned_ranges: Vec::new(),
        ewma_throughput: 50.0, // 50 bytes/s - very slow
        retry_count: 0,
        total_length: 0,
        active: true,
        completed_length: 0,
        recent_bytes_downloaded: 0,
        target_concurrency: 4,
    };

    let meta = MetaTask {
        id: meta_id,
        tenant_id: None,
        name: "slow_task".to_string(),
        total_length: 1000,
        completed_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub],
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        stall_ticks: 0,
        created_at: Some(chrono::Utc::now()),
        etag: None,
        last_modified: None,
        selected_files: None,
        seed_ratio_override: None,
        seed_time_override: None,
        recheck_progress: 0.0,
    };

    orch.tasks.insert(meta_id, meta);

    // Run scaling logic
    orch.perform_adaptive_scaling().await;

    let updated_meta = orch.tasks.get(&meta_id).unwrap();
    assert!(updated_meta.subtasks[0].target_concurrency > 4);
}
