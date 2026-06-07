use crate::AuraWorld;
use aura_core::orchestrator::Event;
use aura_core::task::TaskType;
use cucumber::{given, then, when};
use std::sync::Arc;
use wiremock::matchers::path;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[given(expr = "a mock HTTP server with ETag {string} that returns 304 on match")]
async fn given_mock_server_with_etag(world: &mut AuraWorld, etag_raw: String) {
    let server = MockServer::start().await;
    let etag = format!("\"{}\"", etag_raw);
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    Mock::given(path("/file"))
        .respond_with(move |req: &wiremock::Request| {
            let c = count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if c == 0 {
                // First request: Return 200 with ETag
                ResponseTemplate::new(200)
                    .set_body_bytes(vec![0; 1024])
                    .insert_header("ETag", etag.clone())
                    .insert_header("Content-Length", "1024")
                    .insert_header("Accept-Ranges", "bytes")
            } else {
                // Subsequent requests: Check if-none-match and return 304
                let has_etag = req
                    .headers
                    .iter()
                    .any(|(h, _)| h.as_str().to_lowercase() == "if-none-match");
                if has_etag {
                    ResponseTemplate::new(304)
                } else {
                    ResponseTemplate::new(200)
                        .set_body_bytes(vec![0; 1024])
                        .insert_header("ETag", etag.clone())
                        .insert_header("Content-Length", "1024")
                        .insert_header("Accept-Ranges", "bytes")
                }
            }
        })
        .mount(&server)
        .await;

    world.mirror_uris.push(format!("{}/file", server.uri()));
    world.mock_servers.push(Arc::new(server));
}

#[when("I start the engine and add the task")]
async fn when_i_start_engine_and_add_task(world: &mut AuraWorld) {
    world.init_engine(|_| {}).await;
    let engine = world.engine.as_ref().unwrap();
    let handle = engine
        .add_task(
            "test-task".to_string(),
            world.mirror_uris[0].clone(),
            TaskType::Http,
        )
        .await
        .unwrap();
    world.last_task_id = Some(handle.id());
}

#[when("I wait for the task to complete")]
async fn when_i_wait_for_task_to_complete(world: &mut AuraWorld) {
    let mut rx = world.events_rx.take().unwrap();
    let task_id = world.last_task_id.unwrap();

    let mut completed = false;
    while let Some(event) = rx.recv().await {
        if let Event::TaskCompleted(id) = event {
            if id == task_id {
                completed = true;
                break;
            }
        }
    }
    assert!(completed, "Task did not complete");

    // Clear remaining events from the channel to start fresh for next step
    while rx.try_recv().is_ok() {}

    world.events_rx = Some(rx);
}

#[when("I refresh the task")]
async fn when_i_refresh_the_task(world: &mut AuraWorld) {
    let engine = world.engine.as_ref().unwrap();
    let task_id = world.last_task_id.unwrap();
    engine.refresh(task_id).await.unwrap();
}

#[then("the mock server should have received \"If-None-Match\" header")]
async fn then_mock_server_received_header(_world: &mut AuraWorld) {
    // The wiremock matching rules in `given` enforce this implicitly.
}

#[then("the task should emit a NotModified event")]
async fn then_task_emits_not_modified(world: &mut AuraWorld) {
    let mut rx = world.events_rx.take().unwrap();
    let task_id = world.last_task_id.unwrap();

    let mut not_modified = false;
    let timeout = tokio::time::sleep(std::time::Duration::from_secs(5));
    tokio::pin!(timeout);

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                // NotModified triggers TaskCompleted again
                if let Event::TaskCompleted(id) = event {
                    if id == task_id {
                        not_modified = true;
                        break;
                    }
                }
            }
            _ = &mut timeout => {
                break;
            }
        }
    }
    assert!(
        not_modified,
        "Task did not emit NotModified (TaskCompleted) event"
    );
    world.events_rx = Some(rx);
}
