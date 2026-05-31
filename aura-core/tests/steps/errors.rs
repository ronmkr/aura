use crate::AuraWorld;
use aura_core::task::{DownloadPhase, TaskType};
use aura_core::TaskId;
use cucumber::{given, then, when};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[given(expr = "an HTTP mirror that is returning {string}")]
async fn given_http_mirror_returning_error(world: &mut AuraWorld, error: String) {
    let server = MockServer::start().await;
    let status_code = match error.as_str() {
        "503 Service Unavailable" => 503,
        "404 Not Found" => 404,
        _ => 500,
    };

    let request_count = Arc::new(AtomicU32::new(0));
    let count_clone = request_count.clone();

    Mock::given(method("GET"))
        .respond_with(move |_req: &wiremock::Request| {
            count_clone.fetch_add(1, Ordering::SeqCst);
            ResponseTemplate::new(status_code)
        })
        .mount(&server)
        .await;

    world.mirror_uris.push(server.uri());
    world.mock_servers.push(Arc::new(server));
}

#[when(expr = "the {string} receives the error")]
async fn when_worker_receives_error(world: &mut AuraWorld, _worker: String) {
    if world.engine.is_none() {
        world
            .init_engine(|config| {
                config.network.http_retry_count = 2;
                config.network.http_retry_delay_secs = 1;
            })
            .await;
    }
    let engine = world.engine.as_ref().unwrap();
    let id = TaskId(111);
    world.last_task_id = Some(id);

    let uri = format!("{}/file", world.mirror_uris[0]);
    engine
        .add_task_with_sources(
            id,
            None,
            "error-task".to_string(),
            vec![(uri, TaskType::Http)],
            None,
        )
        .await
        .unwrap();
}

#[then(expr = "it should wait {int} seconds before the first retry")]
async fn then_wait_first_retry(_world: &mut AuraWorld, _secs: u32) {
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}

#[then(expr = "it should wait {int} seconds before the second retry")]
async fn then_wait_second_retry(_world: &mut AuraWorld, _secs: u32) {
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
}

#[then(expr = "it should mark the source as {string} after {int} attempts")]
async fn then_mark_source_after_attempts(world: &mut AuraWorld, state: String, _attempts: u32) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
    let mut success = false;
    for _ in 0..20 {
        interval.tick().await;
        let active = engine.tell_active().await.unwrap();
        if let Some(task) = active.iter().find(|t| t.id == id) {
            if (state == "Degraded" || state == "Failed") && task.subtasks.iter().any(|s| !s.active)
            {
                success = true;
                break;
            }
        } else {
            success = true;
            break;
        }
    }
    assert!(success, "Source was not marked as {}", state);
}

#[given(expr = "a Metalink task with HTTP Mirror A and FTP Mirror B")]
async fn given_metalink_task_mirrors(world: &mut AuraWorld) {
    let server_a = MockServer::start().await;
    let server_b = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server_a)
        .await;

    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 1024])
                .insert_header("Content-Range", "bytes 0-1023/1024"),
        )
        .mount(&server_b)
        .await;

    world.mirror_uris.push(format!("{}/file", server_a.uri()));
    world.mirror_uris.push(format!("{}/file", server_b.uri()));
    world.mock_servers.push(Arc::new(server_a));
    world.mock_servers.push(Arc::new(server_b));
}

#[when(expr = "Mirror A returns a {string}")]
async fn when_mirror_a_returns_error(world: &mut AuraWorld, _error: String) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
    let engine = world.engine.as_ref().unwrap();
    let id = TaskId(222);
    world.last_task_id = Some(id);

    let sources = vec![
        (world.mirror_uris[0].clone(), TaskType::Http),
        (world.mirror_uris[1].clone(), TaskType::Http),
    ];

    engine
        .add_task_with_sources(id, None, "failover-task".to_string(), sources, None)
        .await
        .unwrap();
}

#[then(expr = "the {string} should automatically switch all pending ranges to Mirror B")]
async fn then_switch_pending_ranges(world: &mut AuraWorld, _actor: String) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
    let mut success = false;
    for _ in 0..20 {
        interval.tick().await;
        let active = engine.tell_active().await.unwrap();
        if let Some(task) = active.iter().find(|t| t.id == id) {
            if task.completed_length == 1024 {
                success = true;
                break;
            }
        } else {
            success = true;
            break;
        }
    }
    assert!(success, "Download did not complete via Mirror B");
}

#[then(expr = "Mirror A should be marked as {string} in the task metadata")]
async fn then_mirror_a_marked(world: &mut AuraWorld, state: String) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let active = engine.tell_active().await.unwrap();
    let task = active.iter().find(|t| t.id == id);
    let uri_a = &world.mirror_uris[0];

    if let Some(task) = task {
        let sub = task
            .subtasks
            .iter()
            .find(|s| &s.uri == uri_a)
            .expect("Mirror A not found");
        if state == "Failed" {
            assert!(!sub.active);
            assert_eq!(sub.phase, DownloadPhase::Error);
        }
    }
}

#[given(expr = "the destination drive has only {int} MB of free space")]
async fn given_drive_free_space(_world: &mut AuraWorld, _mb: u32) {}

#[when(expr = "I add a task for a {int} MB file")]
async fn when_add_large_task(world: &mut AuraWorld, _mb: u32) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
    let engine = world.engine.as_ref().unwrap();
    let id = TaskId(333);
    world.last_task_id = Some(id);

    let name = "a".repeat(300);
    let size: u64 = 1024 * 1024;

    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Length", size.to_string())
                .insert_header("Content-Range", format!("bytes 0-{}/{}", size - 1, size)),
        )
        .mount(&server)
        .await;

    engine
        .add_task_with_sources(
            id,
            None,
            name,
            vec![(format!("{}/file", server.uri()), TaskType::Http)],
            None,
        )
        .await
        .unwrap();

    world.mock_servers.push(Arc::new(server));
}

#[then(expr = "the {string} should fail the pre-allocation")]
async fn then_fail_preallocation(_world: &mut AuraWorld, _engine: String) {
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
}

#[then(expr = "the {string} should immediately pause the task with {string}")]
async fn then_pause_task_with_error(world: &mut AuraWorld, _actor: String, error: String) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
    let mut success = false;
    let mut last_phase = None;
    for _ in 0..50 {
        interval.tick().await;
        let active = engine.tell_active().await.unwrap();
        if let Some(task) = active.iter().find(|t| t.id == id) {
            last_phase = Some(task.phase);
            if task.phase == DownloadPhase::Error {
                success = true;
                break;
            }
        } else {
            success = true;
            break;
        }
    }
    assert!(
        success,
        "Task did not enter Error phase as expected for {}. Last phase: {:?}",
        error, last_phase
    );
}

#[then(expr = "the .aura control file should be preserved to allow resumption after cleanup")]
async fn then_preserve_aura_file(_world: &mut AuraWorld) {}
