use crate::orchestrator::test_helpers::create_test_orchestrator as make_test_orchestrator;
use crate::task::{DownloadPhase, MetaTask, SubTask, TaskType};
use crate::TaskId;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::test]
async fn test_adaptive_scaling_min_connections() {
    let (mut orchestrator, _storage_rx) = make_test_orchestrator();

    // Setup a task with a slow subtask
    let sub_id = TaskId(11);
    let sub_task = SubTask {
        id: sub_id,
        uri: "http://slow-mirror.com".to_string(),
        task_type: TaskType::Http,
        phase: DownloadPhase::Downloading,
        assigned_ranges: Vec::new(),
        ewma_throughput: 100.0, // Slow
        retry_count: 0,
        total_length: 0,
        active: true,
        completed_length: 0,
        recent_bytes_downloaded: 0,
        target_concurrency: 10,
    };

    let task = MetaTask {
        id: TaskId(1),
        tenant_id: None,
        name: "test".to_string(),
        total_length: 1000,
        completed_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub_task],
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
    };

    orchestrator.tasks.insert(TaskId(1), task);

    // Run scaling
    orchestrator.perform_adaptive_scaling().await;

    // Assert target concurrency increased
    let scaled_task = orchestrator.tasks.get(&TaskId(1)).unwrap();
    assert_eq!(scaled_task.subtasks[0].target_concurrency, 12);
}

#[tokio::test]
async fn test_check_seed_limits() {
    let (mut orchestrator, _storage_rx) = make_test_orchestrator();

    let sub_task1 = SubTask {
        id: TaskId(11),
        uri: "http://test1".to_string(),
        task_type: TaskType::BitTorrent,
        phase: DownloadPhase::Complete,
        assigned_ranges: Vec::new(),
        ewma_throughput: 0.0,
        retry_count: 0,
        total_length: 1000,
        active: false,
        completed_length: 1000,
        recent_bytes_downloaded: 0,
        target_concurrency: 1,
    };

    let mut task1 = MetaTask {
        id: TaskId(1),
        tenant_id: None,
        name: "test1".to_string(),
        total_length: 1000,
        completed_length: 1000,
        phase: DownloadPhase::Complete,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub_task1],
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
    };

    let bt1 = crate::worker::bittorrent::task::BtTask::from_magnet(
        crate::worker::bittorrent::task::BtTaskFromMagnetArgs {
            id: TaskId(1),
            info_hash: crate::InfoHash::V1([0; 20]),
            dht_tx: tokio::sync::mpsc::channel(1).0,
            lpd_tx: tokio::sync::mpsc::channel(1).0,
            db: orchestrator.db.clone(),
            resource_governor: orchestrator.resource_governor.clone(),
            tenant_id: None,
            config: orchestrator.config.clone(),
            streaming_mode: false,
        },
    );
    bt1.state
        .uploaded_length
        .store(2000, std::sync::atomic::Ordering::Relaxed);
    task1.extensions.insert(
        crate::worker::bittorrent::BT_EXTENSION_KEY.to_string(),
        Arc::new(bt1),
    );

    let sub_task2 = SubTask {
        id: TaskId(12),
        uri: "http://test2".to_string(),
        task_type: TaskType::BitTorrent,
        phase: DownloadPhase::Complete,
        assigned_ranges: Vec::new(),
        ewma_throughput: 0.0,
        retry_count: 0,
        total_length: 1000,
        active: false,
        completed_length: 1000,
        recent_bytes_downloaded: 0,
        target_concurrency: 1,
    };

    let mut task2 = MetaTask {
        id: TaskId(2),
        tenant_id: None,
        name: "test2".to_string(),
        total_length: 1000,
        completed_length: 1000,
        phase: DownloadPhase::Complete,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub_task2],
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
    };

    let bt2 = crate::worker::bittorrent::task::BtTask::from_magnet(
        crate::worker::bittorrent::task::BtTaskFromMagnetArgs {
            id: TaskId(2),
            info_hash: crate::InfoHash::V1([0; 20]),
            dht_tx: tokio::sync::mpsc::channel(1).0,
            lpd_tx: tokio::sync::mpsc::channel(1).0,
            db: orchestrator.db.clone(),
            resource_governor: orchestrator.resource_governor.clone(),
            tenant_id: None,
            config: orchestrator.config.clone(),
            streaming_mode: false,
        },
    );
    *bt2.state.seeding_start_time.lock().unwrap() =
        Some(chrono::Utc::now() - chrono::Duration::seconds(10));
    task2.extensions.insert(
        crate::worker::bittorrent::BT_EXTENSION_KEY.to_string(),
        Arc::new(bt2),
    );

    let sub_task3 = SubTask {
        id: TaskId(13),
        uri: "http://test3".to_string(),
        task_type: TaskType::BitTorrent,
        phase: DownloadPhase::Complete,
        assigned_ranges: Vec::new(),
        ewma_throughput: 0.0,
        retry_count: 0,
        total_length: 1000,
        active: false,
        completed_length: 1000,
        recent_bytes_downloaded: 0,
        target_concurrency: 1,
    };

    let mut task3 = MetaTask {
        id: TaskId(3),
        tenant_id: None,
        name: "test3".to_string(),
        total_length: 1000,
        completed_length: 1000,
        phase: DownloadPhase::Complete,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub_task3],
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
    };

    let bt3 = crate::worker::bittorrent::task::BtTask::from_magnet(
        crate::worker::bittorrent::task::BtTaskFromMagnetArgs {
            id: TaskId(3),
            info_hash: crate::InfoHash::V1([0; 20]),
            dht_tx: tokio::sync::mpsc::channel(1).0,
            lpd_tx: tokio::sync::mpsc::channel(1).0,
            db: orchestrator.db.clone(),
            resource_governor: orchestrator.resource_governor.clone(),
            tenant_id: None,
            config: orchestrator.config.clone(),
            streaming_mode: false,
        },
    );
    bt3.state
        .uploaded_length
        .store(600, std::sync::atomic::Ordering::Relaxed);
    *bt3.state.seed_ratio.lock().unwrap() = Some(0.5);
    task3.extensions.insert(
        crate::worker::bittorrent::BT_EXTENSION_KEY.to_string(),
        Arc::new(bt3),
    );

    orchestrator.tasks.insert(TaskId(1), task1);
    orchestrator.tasks.insert(TaskId(2), task2);
    orchestrator.tasks.insert(TaskId(3), task3);

    // Setup global config:
    // global min_ratio = 1.0
    // global max_seeding_time = 5 seconds (stop_on_either = true)
    {
        let mut config = crate::Config::default();
        config.bittorrent.seeding.min_ratio = 1.0;
        config.bittorrent.seeding.max_seeding_time_secs = 5;
        config.bittorrent.seeding.stop_on_either = true;
        orchestrator.config.store(Arc::new(config));
    }

    orchestrator.check_seed_limits().await;

    // Task 1 should be paused (ratio reached: 2.0 >= 1.0)
    assert_eq!(
        orchestrator.tasks.get(&TaskId(1)).unwrap().phase,
        DownloadPhase::Paused
    );

    // Task 2 should be paused (time reached: 10s >= 5s)
    assert_eq!(
        orchestrator.tasks.get(&TaskId(2)).unwrap().phase,
        DownloadPhase::Paused
    );

    // Task 3 should be paused (override ratio reached: 0.6 >= 0.5)
    assert_eq!(
        orchestrator.tasks.get(&TaskId(3)).unwrap().phase,
        DownloadPhase::Paused
    );
}
