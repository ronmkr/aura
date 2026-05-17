use crate::AuraWorld;
use cucumber::{given, then, when};
use std::io::Write;
use tokio::fs;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[given(expr = "a hook script {string} that writes task ID to {string}")]
async fn given_hook_script(_world: &mut AuraWorld, filename: String, output_file: String) {
    let script_content = format!("#!/bin/sh\necho $1 > {}", output_file);
    let mut file = std::fs::File::create(&filename).unwrap();
    file.write_all(script_content.as_bytes()).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = file.metadata().unwrap().permissions();
        perms.set_mode(0o755);
        file.set_permissions(perms).unwrap();
    }
}

#[given(expr = "the configuration {string} is set to {string}")]
async fn given_hook_config(world: &mut AuraWorld, key: String, value: String) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
    let engine = world.engine.as_ref().unwrap();
    let mut config = (*engine.tell_config().await.unwrap()).clone();

    match key.as_str() {
        "on_download_start" => config.hooks.on_download_start = Some(value),
        "on_download_complete" => config.hooks.on_download_complete = Some(value),
        "on_download_error" => config.hooks.on_download_error = Some(value),
        _ => panic!("Unknown hook config key: {}", key),
    }

    engine.reload_config(config).await.unwrap();
}

#[when(expr = "a download task completes")]
async fn when_task_completes(world: &mut AuraWorld) {
    let mock = MockServer::start().await;
    Mock::given(wiremock::matchers::method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 1024])
                .insert_header("Content-Range", "bytes 0-1023/1024"),
        )
        .mount(&mock)
        .await;

    let uri = format!("{}/file", mock.uri());

    let engine = world.engine.as_ref().unwrap();
    let id = aura_core::TaskId(999);
    world.last_task_id = Some(id);

    let mut rx = engine.subscribe();

    engine
        .add_task_with_sources(
            id,
            "hook-test".to_string(),
            vec![(uri, aura_core::task::TaskType::Http)],
        )
        .await
        .unwrap();

    // Wait for completion
    while let Ok(event) = rx.recv().await {
        match event {
            aura_core::orchestrator::Event::TaskCompleted(ev_id) if ev_id == id => {
                break;
            }
            aura_core::orchestrator::Event::TaskError { id: ev_id, message: err } if ev_id == id => {
                panic!("Task failed instead of completing: {}", err);
            }
            _ => {}
        }
    }
}

#[then(expr = "the file {string} should contain the task ID")]
async fn then_file_contains_id(world: &mut AuraWorld, filename: String) {
    let id = world.last_task_id.unwrap();

    // Wait for hook to execute (it's async tokio::spawn)
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let content = fs::read_to_string(&filename).await.unwrap();
    assert!(content.contains(&id.0.to_string()));

    // Cleanup
    let _ = fs::remove_file(&filename).await;
    let _ = fs::remove_file("notify.sh").await;
}
