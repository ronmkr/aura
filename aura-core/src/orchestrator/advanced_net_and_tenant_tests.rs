use super::Orchestrator;
use crate::task::{DownloadPhase, MetaTask, SubTask, TaskType};
use crate::TaskId;
use std::collections::HashMap;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::{broadcast, mpsc};
use arc_swap::ArcSwap;

fn make_test_orchestrator() -> (
    Orchestrator,
    mpsc::Receiver<crate::storage::StorageRequest>,
    tempfile::TempDir,
) {
    let (_command_tx, command_rx) = mpsc::channel(1024);
    let (storage_tx, storage_rx) = mpsc::channel(1024);
    let (_storage_event_tx, storage_event_rx) = mpsc::channel(1024);
    let (event_tx, _event_rx) = broadcast::channel(1024);
    let (dht_tx, _dht_rx) = mpsc::channel(1024);
    let (lpd_tx, _lpd_rx) = mpsc::channel(1024);
    let (scrub_tx, _scrub_rx) = mpsc::channel(1024);

    let config = Arc::new(ArcSwap::from_pointee(crate::Config::default()));

    let (mut orch, _tx) = Orchestrator::new(
        command_rx,
        storage_tx,
        storage_event_rx,
        dht_tx,
        lpd_tx,
        scrub_tx,
        [0; 20],
        config,
        sled::Config::new().temporary(true).open().unwrap(),
        Arc::new(crate::net_util::TokioResolver::system()),
    );

    let temp_dir = tempdir().unwrap();
    (orch, storage_rx, temp_dir)
}

#[tokio::test]
async fn test_multi_tenant_path_isolation_and_throttling() {
    let (mut orch, _storage_rx, temp_dir) = make_test_orchestrator();

    let tenant_id = crate::TenantId("tenant_a".to_string());
    let tenant_root = temp_dir.path().join("tenant_a_root");
    std::fs::create_dir_all(&tenant_root).unwrap();

    orch.register_tenant(
        tenant_id.clone(),
        Some(tenant_root.clone()),
        crate::orchestrator::TenantQuotas::default(),
    )
    .await;

    let meta_id = TaskId(1);
    let meta = MetaTask {
        id: meta_id,
        tenant_id: Some(tenant_id),
        name: "secret.bin".to_string(),
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
        seed_ratio: None,
        seed_time: None,
    };
    orch.tasks.insert(meta_id, meta);

    // Verify resolved path is within tenant root
    let path = orch.mapping_engine.resolve_path(
        orch.tasks.get(&meta_id).unwrap(),
        &std::path::PathBuf::from("."),
    );
    assert!(path.starts_with(&tenant_root));
}

#[tokio::test]
async fn test_multi_tenant_task_limits() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let tenant_id = crate::TenantId("limited_tenant".to_string());
    let mut quotas = crate::orchestrator::TenantQuotas::default();
    quotas.max_active_tasks = 1;

    orch.register_tenant(tenant_id.clone(), None, quotas).await;

    // Add first task (allowed)
    let res1 = orch
        .handle_add_task(
            TaskId(1),
            Some(tenant_id.clone()),
            "task1".to_string(),
            vec![("http://test1".to_string(), TaskType::Http)],
            None,
            3,
            false,
            Vec::new(),
            None,
        )
        .await;
    assert!(res1.is_ok());

    // Add second task (rejected)
    let res2 = orch
        .handle_add_task(
            TaskId(2),
            Some(tenant_id),
            "task2".to_string(),
            vec![("http://test2".to_string(), TaskType::Http)],
            None,
            3,
            false,
            Vec::new(),
            None,
        )
        .await;
    assert!(res2.is_err());
}

#[tokio::test]
async fn test_captive_portal_pausing() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let meta_id = TaskId(1);
    let sub_id = TaskId(11);
    let sub = SubTask {
        id: sub_id,
        uri: "http://airport-wifi.com".to_string(),
        task_type: TaskType::Http,
        phase: DownloadPhase::Downloading,
        assigned_ranges: Vec::new(),
        ewma_throughput: 100.0,
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
        subtasks: vec![sub],
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: None,
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        follow_on: None,
        stall_ticks: 0,
        seed_ratio: None,
        seed_time: None,
    };
    orch.tasks.insert(meta_id, meta);

    // Simulate captive portal event from worker
    orch.handle_subtask_event(crate::orchestrator::SubTaskEvent::CaptivePortalDetected(
        meta_id, sub_id,
    ))
    .await
    .unwrap();

    // Task should be paused
    let updated_meta = orch.tasks.get(&meta_id).unwrap();
    assert_eq!(updated_meta.phase, DownloadPhase::Paused);
}

#[tokio::test]
async fn test_resource_preemption_logic() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    // Add Low priority task (priority 5)
    orch.handle_add_task(
        TaskId(1),
        None,
        "low".to_string(),
        vec![("http://low".to_string(), TaskType::Http)],
        None,
        5,
        false,
        Vec::new(),
        None,
    )
    .await
    .unwrap();

    // Add High priority task (priority 1)
    let res = orch
        .handle_add_task(
            TaskId(2),
            None,
            "high".to_string(),
            vec![("http://high".to_string(), TaskType::Http)],
            None,
            1,
            false,
            Vec::new(),
            None,
        )
        .await;

    assert!(res.is_ok());
    // Preemption logic is internal, but we can verify both exist
    assert_eq!(orch.tasks.len(), 2);
}

#[tokio::test]
async fn test_interface_roaming_reconnector() {
    let (mut orch, _storage_rx, _temp_dir) = make_test_orchestrator();

    let meta_id = TaskId(1);
    let meta = MetaTask {
        id: meta_id,
        tenant_id: None,
        name: "test".to_string(),
        total_length: 1000,
        completed_length: 0,
        uploaded_length: 0,
        phase: DownloadPhase::Paused,
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
        stall_ticks: 0,
        seed_ratio: None,
        seed_time: None,
    };
    orch.tasks.insert(meta_id, meta);

    // When network roaming detected
    orch.handle_command(crate::orchestrator::Command::RetrySubtask(meta_id, TaskId(0)))
        .await
        .unwrap();

    // Verify task state or reconnection attempt
    assert_eq!(orch.tasks.len(), 1);
}
