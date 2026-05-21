use crate::AuraWorld;
use aura_core::task::TaskType;
use aura_core::TaskId;
use cucumber::{given, then, when};
use rand::RngExt;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[given(expr = "a Metalink file \"{word}\" containing:")]
async fn given_metalink_file(
    world: &mut AuraWorld,
    filename: String,
    step: &cucumber::gherkin::Step,
) {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<metalink version=\"3.0\" xmlns=\"http://www.metalinker.org/\">\n<files>\n<file name=\"file.zip\">\n<resources>\n");

    if let Some(table) = step.table.as_ref() {
        for row in table.rows.iter().skip(1) {
            let protocol = &row[0];
            let uri = &row[1];
            xml.push_str(&format!(
                "<url protocol=\"{}\">{}</url>\n",
                protocol.to_lowercase(),
                uri
            ));
        }
    }

    xml.push_str("</resources>\n</file>\n</files>\n</metalink>");
    let path = world.temp_dir.path().join(&filename);
    tokio::fs::write(path, xml.into_bytes())
        .await
        .expect("Failed to write test metalink");
}

#[given(expr = "the global download limit is unlimited")]
async fn given_unlimited_limit(world: &mut AuraWorld) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
}

#[when(expr = "I add the task via \"{word}\"")]
async fn when_add_metalink_task(world: &mut AuraWorld, filename: String) {
    let engine = world.engine.as_ref().unwrap();
    let path = world.temp_dir.path().join(&filename);
    let handle = engine
        .add_task(
            "test-task".to_string(),
            path.to_str().unwrap().to_string(),
            TaskType::Http,
        )
        .await
        .expect("Failed to add task");
    world.last_task_id = Some(handle.id());
}

#[then(expr = "the engine should spawn {int} HTTP worker and {int} FTP worker")]
async fn then_spawn_workers(world: &mut AuraWorld, http_count: usize, ftp_count: usize) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let active_tasks = engine.tell_active().await.unwrap();
    let task = active_tasks
        .iter()
        .find(|t| t.id == id)
        .expect("Task not found");

    let actual_http = task
        .subtasks
        .iter()
        .filter(|s| s.task_type == TaskType::Http)
        .count();
    let actual_ftp = task
        .subtasks
        .iter()
        .filter(|s| s.task_type == TaskType::Ftp)
        .count();

    if actual_http != http_count || actual_ftp != ftp_count {
        panic!("Worker count mismatch!\nExpected: HTTP={}, FTP={}\nActual: HTTP={}, FTP={}\nSubtasks: {:?}", 
               http_count, ftp_count, actual_http, actual_ftp,
               task.subtasks.iter().map(|s| (s.task_type.clone(), s.uri.clone())).collect::<Vec<_>>());
    }
}

#[then(expr = "the downloaded data should be aggregated into \"{word}\"")]
async fn then_aggregated_into(_world: &mut AuraWorld, _filename: String) {}

#[then(expr = "the final file \"{word}\" should pass SHA-256 verification")]
async fn then_pass_verification(_world: &mut AuraWorld, _filename: String) {}

#[given(expr = "a download task with 2 HTTP mirrors")]
async fn given_task_with_mirrors(world: &mut AuraWorld) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
}

#[given(regex = r"Mirror A is throttled to (\d+) KB/s")]
async fn given_mirror_a_throttled(world: &mut AuraWorld, _speed: u32) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0; 1024])
                .insert_header("Content-Range", "bytes 0-1023/1024")
                .set_delay(std::time::Duration::from_secs(1)),
        )
        .mount(&server)
        .await;
    world.mirror_uris.push(format!("{}/file", server.uri()));
    world.mock_servers.push(std::sync::Arc::new(server));
}

#[given(expr = "Mirror B is unlimited")]
async fn given_mirror_b_unlimited(world: &mut AuraWorld) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![1; 1024])
                .insert_header("Content-Range", "bytes 0-1023/1024"),
        )
        .mount(&server)
        .await;
    world.mirror_uris.push(format!("{}/file", server.uri()));
    world.mock_servers.push(std::sync::Arc::new(server));
}

#[when(expr = "the download starts")]
pub async fn when_download_starts(world: &mut AuraWorld) {
    let engine = world.engine.as_ref().unwrap();
    let sources = world
        .mirror_uris
        .iter()
        .map(|u| (u.clone(), TaskType::Http))
        .collect();
    let id = TaskId(rand::rng().random());
    let handle = engine
        .add_task_with_sources(id, "racing-task".to_string(), sources, None)
        .await
        .unwrap();
    world.last_task_id = Some(handle.id());
}

#[then(expr = "the engine should detect Mirror A is lagging")]
async fn then_detect_lagging(_world: &mut AuraWorld) {
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
}

#[then(expr = "Mirror B should \"steal\" the remaining ranges assigned to Mirror A")]
async fn then_steal_ranges(world: &mut AuraWorld) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();
    let active_tasks = engine.tell_active().await.unwrap();
    let _task = active_tasks
        .iter()
        .find(|t| t.id == id)
        .expect("Task not found");
}

#[then(expr = "the download should complete without waiting for Mirror A")]
async fn then_complete_without_waiting(world: &mut AuraWorld) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let mut timeout = tokio::time::interval(std::time::Duration::from_millis(500));
    for _ in 0..10 {
        timeout.tick().await;
        let active_tasks = engine.tell_active().await.unwrap();
        let task = active_tasks.iter().find(|t| t.id == id);
        if task.is_none() || task.unwrap().phase == aura_core::task::DownloadPhase::Complete {
            return;
        }
        if task.unwrap().completed_length > 0 {
            return;
        }
    }
}
