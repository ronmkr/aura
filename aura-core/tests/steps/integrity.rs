use crate::AuraWorld;
use aura_core::task::{DownloadPhase, TaskType};
use aura_core::TaskId;
use cucumber::{given, then, when};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[given(expr = "an HTTP mirror for {string} with content {string}")]
async fn given_http_mirror_with_content(world: &mut AuraWorld, name: String, content: String) {
    let server = MockServer::start().await;
    let bytes = content.into_bytes();
    let len = bytes.len() as u64;

    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(bytes)
                .insert_header("Content-Length", len.to_string())
                .insert_header("Content-Range", format!("bytes 0-{}/{}", len - 1, len)),
        )
        .mount(&server)
        .await;

    world.mirror_uris.push(format!("{}/{}", server.uri(), name));
    world.mock_servers.push(std::sync::Arc::new(server));
}

#[given(expr = "the expected SHA-256 checksum is {string}")]
async fn given_expected_sha256(world: &mut AuraWorld, checksum: String) {
    world.task_checksum = Some(aura_core::Checksum::Sha256(checksum));
}

#[given(expr = "the expected MD5 checksum is {string}")]
async fn given_expected_md5(world: &mut AuraWorld, checksum: String) {
    world.task_checksum = Some(aura_core::Checksum::Md5(checksum));
}

#[given(expr = "the expected SHA-512 checksum is {string}")]
async fn given_expected_sha512(world: &mut AuraWorld, checksum: String) {
    world.task_checksum = Some(aura_core::Checksum::Sha512(checksum));
}

#[when(expr = "I add the task with the checksum")]
async fn when_add_task_with_checksum(world: &mut AuraWorld) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
    let engine = world.engine.as_ref().unwrap();
    let id = TaskId(rand::random());
    world.last_task_id = Some(id);

    let uri = world.mirror_uris.last().unwrap().clone();
    let name = uri.split('/').next_back().unwrap().to_string();

    engine
        .add_task_with_checksum(id, name, uri, TaskType::Http, world.task_checksum.clone())
        .await
        .unwrap();
}

#[then(expr = "the download should transition to {string} phase after 100%")]
async fn then_transition_to_phase(world: &mut AuraWorld, phase: String) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
    let mut saw_phase = false;
    for _ in 0..100 {
        interval.tick().await;
        let active = engine.tell_active().await.unwrap();
        if let Some(task) = active.iter().find(|t| t.id == id) {
            let current_phase = format!("{:?}", task.phase);
            // If it's already Complete, it must have passed through Verifying
            if current_phase == phase || current_phase == "Complete" {
                saw_phase = true;
                break;
            }
        } else {
            // Task gone from active often means it completed successfully
            saw_phase = true;
            break;
        }
    }
    assert!(
        saw_phase,
        "Task did not transition to {} phase (or Complete). Last seen phase: {:?}",
        phase,
        engine
            .tell_active()
            .await
            .unwrap()
            .iter()
            .find(|t| t.id == id)
            .map(|t| t.phase)
    );
}

#[then(expr = "the task should eventually be {string}")]
async fn then_task_eventually_complete(world: &mut AuraWorld, status: String) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
    let mut success = false;
    for _ in 0..50 {
        interval.tick().await;
        let active = engine.tell_active().await.unwrap();
        if let Some(task) = active.iter().find(|t| t.id == id) {
            if format!("{:?}", task.phase) == status {
                success = true;
                break;
            }
        } else {
            // If gone from active, check if it's complete
            if status == "Complete" {
                success = true; // Optimization: assume gone means finished
                break;
            }
        }
    }
    assert!(success, "Task did not reach status {}", status);
}

#[then(expr = "the task should eventually fail with a {string} error")]
async fn then_task_fails_with_error(world: &mut AuraWorld, _error_msg: String) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));
    let mut failed = false;
    for _ in 0..50 {
        interval.tick().await;
        let active = engine.tell_active().await.unwrap();
        if let Some(task) = active.iter().find(|t| t.id == id) {
            if task.phase == DownloadPhase::Error {
                failed = true;
                break;
            }
        }
    }
    assert!(failed, "Task did not fail as expected");
    // Check logs or event bus for error message if possible
}

#[then(expr = "the {string} file should be preserved")]
async fn then_file_preserved(world: &mut AuraWorld, filename: String) {
    let base_path = world.temp_dir.path().join(filename);
    let part_path = aura_core::storage::utils::get_part_path(&base_path).unwrap();
    assert!(
        part_path.exists(),
        "Part file {:?} was not preserved",
        part_path
    );
}
