use super::Orchestrator;
use crate::net_util::TokioResolver;
use crate::task::{DownloadPhase, MetaTask, SubTask, TaskType};
use crate::TaskId;
use arc_swap::ArcSwap;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

fn make_test_orchestrator() -> (Orchestrator, mpsc::Receiver<crate::storage::StorageRequest>) {
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
            nat_tx: mpsc::channel(1).0, // Add dummy nat_tx
        },
        config,
        sled::Config::new().temporary(true).open().unwrap(),
        Arc::new(TokioResolver::builder_tokio().unwrap().build().unwrap()),
    );

    (orch, storage_rx)
}

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
        uploaded_length: 0,
        phase: DownloadPhase::Downloading,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub_task],
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

    let task1 = MetaTask {
        id: TaskId(1),
        tenant_id: None,
        name: "test1".to_string(),
        total_length: 1000,
        completed_length: 1000,
        uploaded_length: 2000,
        phase: DownloadPhase::Complete,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub_task1],
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: Some(chrono::Utc::now() - chrono::Duration::seconds(10)),
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        stall_ticks: 0,
        created_at: None,
        seed_ratio: None,
        seed_time: None,
    };

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

    let task2 = MetaTask {
        id: TaskId(2),
        tenant_id: None,
        name: "test2".to_string(),
        total_length: 1000,
        completed_length: 1000,
        uploaded_length: 500,
        phase: DownloadPhase::Complete,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub_task2],
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: Some(chrono::Utc::now() - chrono::Duration::seconds(10)),
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        stall_ticks: 0,
        created_at: None,
        seed_ratio: None,
        seed_time: None,
    };

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

    let task3 = MetaTask {
        id: TaskId(3),
        tenant_id: None,
        name: "test3".to_string(),
        total_length: 1000,
        completed_length: 1000,
        uploaded_length: 600,
        phase: DownloadPhase::Complete,
        priority: 3,
        streaming_mode: false,
        range_supported: true,
        follow_on: None,
        subtasks: vec![sub_task3],
        pending_ranges: Vec::new(),
        in_flight_ranges: Vec::new(),
        checksum: None,
        seeding_start_time: Some(chrono::Utc::now() - chrono::Duration::seconds(2)),
        blacklisted_uris: Vec::new(),
        extensions: HashMap::new(),
        depends_on: Vec::new(),
        stall_ticks: 0,
        created_at: None,
        seed_ratio: Some(0.5),
        seed_time: None,
    };

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
