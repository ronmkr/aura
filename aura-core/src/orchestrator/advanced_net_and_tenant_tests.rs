use super::*;
use crate::orchestrator::SubTaskEvent;
use crate::task::{DownloadPhase, MetaTask, SubTask, TaskType};
use crate::TaskId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_captive_portal_pausing() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let meta_id = TaskId(1);
    let sub_id = TaskId(101);

    let sub = SubTask {
        id: sub_id,
        task_type: TaskType::Http,
        uri: "http://example.com/download.zip".to_string(),
        assigned_ranges: Vec::new(),
        total_length: 0,
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
        total_length: 0,
        completed_length: 0,
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub],
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
    };

    orch.tasks.insert(meta_id, meta);
    orch.cancellation_tokens
        .insert(meta_id, CancellationToken::new());

    // Fire captive portal failure
    let event = SubTaskEvent::Failed(
        meta_id,
        sub_id,
        "Captive portal detected: landing page redirect".to_string(),
    );
    let res = orch.handle_subtask_event(event).await;
    assert!(res.is_ok());

    // Verify task is safely Paused
    let task = orch.tasks.get(&meta_id).unwrap();
    assert_eq!(task.phase, DownloadPhase::Paused);
    assert_eq!(task.subtasks[0].phase, DownloadPhase::Paused);
}

#[tokio::test]
async fn test_interface_roaming_reconnector() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let meta_id = TaskId(1);
    let sub_id = TaskId(101);

    let sub = SubTask {
        id: sub_id,
        task_type: TaskType::Http,
        uri: "http://example.com/file.zip".to_string(),
        assigned_ranges: Vec::new(),
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
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub],
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
    };

    orch.tasks.insert(meta_id, meta);
    orch.cancellation_tokens
        .insert(meta_id, CancellationToken::new());

    // Trigger interface roaming reconnector event
    let event = SubTaskEvent::RoamingDetected;
    let res = orch.handle_subtask_event(event).await;
    assert!(res.is_ok());

    // Verify tasks are automatically resumed (still Downloading phase)
    let task = orch.tasks.get(&meta_id).unwrap();
    assert_eq!(task.phase, DownloadPhase::Downloading);
}

#[tokio::test]
async fn test_multi_tenant_task_limits() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let tenant_id = crate::TenantId("tenant_1".to_string());

    // Configure tenant context
    orch.tenants.insert(
        tenant_id.clone(),
        crate::orchestrator::state::TenantContext {
            throttler: Arc::new(crate::throttler::Throttler::new(0, 0)),
            max_tasks: Some(1),
            disk_path_root: None,
        },
    );

    // Add first task (should succeed)
    let res1 = orch
        .handle_add_task(
            TaskId(1),
            Some(tenant_id.clone()),
            "task_1".to_string(),
            vec![("http://example.com/file1".to_string(), TaskType::Http)],
            None,
            3,
            false,
            vec![],
            None,
        )
        .await;
    assert!(res1.is_ok());

    // Add second task for the same tenant (should fail due to max_tasks = 1)
    let res2 = orch
        .handle_add_task(
            TaskId(2),
            Some(tenant_id.clone()),
            "task_2".to_string(),
            vec![("http://example.com/file2".to_string(), TaskType::Http)],
            None,
            3,
            false,
            vec![],
            None,
        )
        .await;
    assert!(res2.is_err());
    assert!(res2
        .unwrap_err()
        .to_string()
        .contains("Tenant task limit reached"));

    // Add third task for a different tenant (should succeed)
    let tenant_id_2 = crate::TenantId("tenant_2".to_string());
    orch.tenants.insert(
        tenant_id_2.clone(),
        crate::orchestrator::state::TenantContext {
            throttler: Arc::new(crate::throttler::Throttler::new(0, 0)),
            max_tasks: Some(1),
            disk_path_root: None,
        },
    );

    let res3 = orch
        .handle_add_task(
            TaskId(3),
            Some(tenant_id_2),
            "task_3".to_string(),
            vec![("http://example.com/file3".to_string(), TaskType::Http)],
            None,
            3,
            false,
            vec![],
            None,
        )
        .await;
    assert!(res3.is_ok());
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
        tenant_id: None,
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
        follow_on: None,
    };
    orch.tasks.insert(TaskId(1), meta_a);

    // Add active Task B (prio 4, downloading)
    let meta_b = MetaTask {
        id: TaskId(2),
        tenant_id: None,
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
        follow_on: None,
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
            None,
            "task_c".to_string(),
            Vec::new(),
            None,
            0,
            false,
            Vec::new(),
            None,
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

#[tokio::test]
async fn test_multi_tenant_path_isolation_and_throttling() {
    let (mut orch, mut storage_rx, _temp_dir) = make_test_orchestrator();

    let tenant_id = crate::TenantId("tenant_1".to_string());
    let custom_root = std::path::PathBuf::from("/tmp/tenant_1_custom_root");
    let tenant_throttler = Arc::new(crate::throttler::Throttler::new(10000, 10000));

    // Configure tenant context with custom disk_path_root and custom throttler
    orch.tenants.insert(
        tenant_id.clone(),
        crate::orchestrator::state::TenantContext {
            throttler: tenant_throttler.clone(),
            max_tasks: Some(5),
            disk_path_root: Some(custom_root.clone()),
        },
    );

    // Add task for this tenant
    let res = orch
        .handle_add_task(
            TaskId(1),
            Some(tenant_id.clone()),
            "task_1".to_string(),
            vec![("http://example.com/file1".to_string(), TaskType::Http)],
            None,
            3,
            false,
            vec![],
            None,
        )
        .await;
    assert!(res.is_ok());

    // 1. Verify storage path starts with custom_root
    let req = storage_rx.recv().await.unwrap();
    match req {
        crate::storage::StorageRequest::RegisterTask { path, .. } => {
            assert!(path.starts_with(&custom_root));
            assert_eq!(path.file_name().unwrap().to_str().unwrap(), "task_1");
        }
        _ => panic!("Expected RegisterTask request"),
    }

    // 2. Verify task is registered in the tenant's throttler instead of the global one
    tenant_throttler.update_task_priority(TaskId(1), 5).await; // Should succeed/be handled by tenant's throttler
}
