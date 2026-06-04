use crate::orchestrator::command::AddTaskArgs;
use crate::orchestrator::state::{Orchestrator, OrchestratorChannels};
use crate::task::TaskType;
use crate::TaskId;
use std::sync::Arc;
use tokio::sync::mpsc;

fn setup_orchestrator(config: crate::Config) -> (Orchestrator, tempfile::TempDir) {
    let (_command_tx, command_rx) = mpsc::channel(100);
    let (storage_tx, _storage_rx) = mpsc::channel(100);
    let (_completion_tx, completion_rx) = mpsc::channel(100);
    let (dht_tx, _dht_rx) = mpsc::channel(100);
    let (nat_tx, _nat_rx) = mpsc::channel(100);
    let (lpd_tx, _lpd_rx) = mpsc::channel(100);

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
        OrchestratorChannels {
            command_rx,
            storage_tx,
            storage_completion_rx: completion_rx,
            dht_tx,
            lpd_tx,
            nat_tx,
        },
        config_swap,
        db,
        dns_resolver,
    );

    (orchestrator, temp_dir)
}

#[tokio::test]
async fn test_add_duplicate_uri_returns_existing_gid() {
    let mut config = crate::Config::default();
    config.limits.allow_duplicate_uris = false;

    let (mut orchestrator, _dir) = setup_orchestrator(config);

    let task_id1 = TaskId(1);
    let args1 = AddTaskArgs {
        id: task_id1,
        tenant_id: None,
        name: "task1".to_string(),
        sources: vec![("http://example.com/file".to_string(), TaskType::Http)],
        checksum: None,
        priority: 3,
        streaming_mode: false,
        depends_on: Vec::new(),
        follow_on: None,
    };

    // First task should be added successfully
    assert!(orchestrator.handle_add_task(args1).await.is_ok());

    // Second task with same URI should fail with DuplicateTask
    let task_id2 = TaskId(2);
    let args2 = AddTaskArgs {
        id: task_id2,
        tenant_id: None,
        name: "task2".to_string(),
        sources: vec![("http://example.com/file".to_string(), TaskType::Http)],
        checksum: None,
        priority: 3,
        streaming_mode: false,
        depends_on: Vec::new(),
        follow_on: None,
    };

    let result = orchestrator.handle_add_task(args2).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        crate::Error::DuplicateTask(existing_id) => {
            assert_eq!(existing_id, task_id1);
        }
        other => panic!("Expected DuplicateTask error, got {:?}", other),
    }
}

#[tokio::test]
async fn test_add_duplicate_allowed_when_config_set() {
    let mut config = crate::Config::default();
    config.limits.allow_duplicate_uris = true;

    let (mut orchestrator, _dir) = setup_orchestrator(config);

    let task_id1 = TaskId(1);
    let args1 = AddTaskArgs {
        id: task_id1,
        tenant_id: None,
        name: "task1".to_string(),
        sources: vec![("http://example.com/file".to_string(), TaskType::Http)],
        checksum: None,
        priority: 3,
        streaming_mode: false,
        depends_on: Vec::new(),
        follow_on: None,
    };

    assert!(orchestrator.handle_add_task(args1).await.is_ok());

    let task_id2 = TaskId(2);
    let args2 = AddTaskArgs {
        id: task_id2,
        tenant_id: None,
        name: "task2".to_string(),
        sources: vec![("http://example.com/file".to_string(), TaskType::Http)],
        checksum: None,
        priority: 3,
        streaming_mode: false,
        depends_on: Vec::new(),
        follow_on: None,
    };

    // Since duplication is allowed, it should succeed
    assert!(orchestrator.handle_add_task(args2).await.is_ok());
}

#[tokio::test]
async fn test_max_active_tasks_limit_enforced() {
    let mut config = crate::Config::default();
    config.limits.max_active_tasks = 2;
    config.limits.allow_duplicate_uris = true; // allow duplicate so we can add easily

    let (mut orchestrator, _dir) = setup_orchestrator(config);

    let args1 = AddTaskArgs {
        id: TaskId(1),
        tenant_id: None,
        name: "task1".to_string(),
        sources: vec![("http://example.com/1".to_string(), TaskType::Http)],
        checksum: None,
        priority: 3,
        streaming_mode: false,
        depends_on: Vec::new(),
        follow_on: None,
    };
    assert!(orchestrator.handle_add_task(args1).await.is_ok());

    let args2 = AddTaskArgs {
        id: TaskId(2),
        tenant_id: None,
        name: "task2".to_string(),
        sources: vec![("http://example.com/2".to_string(), TaskType::Http)],
        checksum: None,
        priority: 3,
        streaming_mode: false,
        depends_on: Vec::new(),
        follow_on: None,
    };
    assert!(orchestrator.handle_add_task(args2).await.is_ok());

    let args3 = AddTaskArgs {
        id: TaskId(3),
        tenant_id: None,
        name: "task3".to_string(),
        sources: vec![("http://example.com/3".to_string(), TaskType::Http)],
        checksum: None,
        priority: 3,
        streaming_mode: false,
        depends_on: Vec::new(),
        follow_on: None,
    };
    let result = orchestrator.handle_add_task(args3).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        crate::Error::TooManyTasks(limit) => {
            assert_eq!(limit, 2);
        }
        other => panic!("Expected TooManyTasks error, got {:?}", other),
    }
}
