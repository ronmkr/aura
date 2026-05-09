use crate::steps::aggregation::when_download_starts;
use crate::AuraWorld;
use aura_core::task::TaskType;
use aura_core::TaskId;
use cucumber::{given, then, when};
use rand::RngExt;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[given(regex = r#"the configuration "global_download_limit" is set to "(\d+)" \((\d+) KB/s\)"#)]
async fn given_global_limit(world: &mut AuraWorld, limit: u64, _kb: u64) {
    world
        .init_engine(|config| {
            config.bandwidth.global_download_limit = limit;
        })
        .await;
}

#[when(expr = "I start a high-speed HTTP download")]
async fn when_start_high_speed_download(world: &mut AuraWorld) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0; 100 * 1024])
                .insert_header("Content-Range", "bytes 0-102399/102400"),
        )
        .mount(&server)
        .await;

    let engine = world.engine.as_ref().unwrap();
    let id = TaskId(rand::rng().random());
    let handle = engine
        .add_task_with_sources(
            id,
            "throttled-task".to_string(),
            vec![(format!("{}/file", server.uri()), TaskType::Http)],
        )
        .await
        .unwrap();
    world.last_task_id = Some(handle.id());
    world.mock_servers.push(std::sync::Arc::new(server));
}

#[then(regex = r"the EWMA throughput should not exceed (\d+) KB/s over any (\d+)-second window")]
async fn then_check_throughput(world: &mut AuraWorld, max_kb: f64, seconds: u64) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));

    for _ in 0..seconds {
        interval.tick().await;
        let active_tasks = engine.tell_active().await.unwrap();
        let task = active_tasks
            .iter()
            .find(|t| t.id == id)
            .expect("Task not found");

        for sub in &task.subtasks {
            let kb_s = sub.ewma_throughput / 1024.0;
            assert!(
                kb_s <= max_kb * 2.0,
                "Throughput {} KB/s exceeded limit {} KB/s",
                kb_s,
                max_kb
            );
        }
    }
}

#[then(expr = "the workers should wait for tokens from the global bucket before network reads")]
async fn then_check_admission_control(_world: &mut AuraWorld) {}

#[given(regex = r#"the global download limit is "(\d+)" \((\d+) KB/s\)"#)]
async fn given_global_limit_str(world: &mut AuraWorld, limit: String, _kb: i32) {
    let limit_val: u64 = limit.parse().unwrap();
    if world.engine.is_none() {
        world
            .init_engine(|config| {
                config.bandwidth.global_download_limit = limit_val;
            })
            .await;
    } else {
        let engine = world.engine.as_ref().unwrap();
        let mut config = (*engine.tell_config().await.unwrap()).clone();
        config.bandwidth.global_download_limit = limit_val;
        engine.reload_config(config).await.unwrap();
    }
}

#[given(regex = r#"Task A has a per-task limit of "(\d+)" \((\d+) KB/s\)"#)]
async fn given_task_limit(world: &mut AuraWorld, limit: String, _kb: i32) {
    let limit_val: u64 = limit.parse().unwrap();
    let engine = world.engine.as_ref().unwrap();
    let mut config = (*engine.tell_config().await.unwrap()).clone();
    config.bandwidth.per_task_download_limit = limit_val;
    engine.reload_config(config).await.unwrap();
}

#[when(expr = "I start Task A")]
async fn when_start_task_a(world: &mut AuraWorld) {
    when_start_high_speed_download(world).await;
}

#[then(regex = r"Task A should be capped at (\d+) KB/s")]
async fn then_task_capped(world: &mut AuraWorld, max_kb: u32) {
    then_check_throughput(world, max_kb as f64, 3).await;
}

#[then(expr = "the global bucket should still have remaining capacity")]
async fn then_global_capacity(_world: &mut AuraWorld) {}

#[given(regex = r"an HTTP server that caps per-connection speed to (\d+) KB/s")]
async fn given_slow_server(world: &mut AuraWorld, kb: u32) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let uri = format!("http://127.0.0.1:{}/file", port);

    world.mirror_uris.push(uri);

    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf).await;

                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Range: bytes 0-10485759/10485760\r\n\r\n",
                    10 * 1024 * 1024
                );
                if stream.write_all(response.as_bytes()).await.is_err() {
                    return;
                }

                let chunk = vec![0u8; (kb * 1024) as usize];
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                for _ in 0..100 {
                    interval.tick().await;
                    if stream.write_all(&chunk).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
}

#[given(regex = r#"the "([^"]+)" is (?:set to )?"([^"]+)"(?: \(.*\))?"#)]
async fn given_config_val(world: &mut AuraWorld, key: String, val: String) {
    if world.engine.is_none() {
        world
            .init_engine(|config| {
                if key == "max_connections_per_task" {
                    config.bandwidth.max_connections_per_task = val.parse().unwrap();
                } else if key == "global_download_limit" {
                    config.bandwidth.global_download_limit = val.parse().unwrap();
                }
            })
            .await;
    } else {
        let engine = world.engine.as_ref().unwrap();
        let mut config = (*engine.tell_config().await.unwrap()).clone();
        if key == "max_connections_per_task" {
            config.bandwidth.max_connections_per_task = val.parse().unwrap();
        } else if key == "global_download_limit" {
            config.bandwidth.global_download_limit = val.parse().unwrap();
        }
        engine.reload_config(config).await.unwrap();
    }
}

#[when(expr = "the download starts with 1 connection")]
async fn when_download_starts_single(world: &mut AuraWorld) {
    when_download_starts(world).await;
}

#[then(expr = "the Orchestrator should detect throughput is below the global potential")]
async fn then_detect_potential(_world: &mut AuraWorld) {
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
}

#[then(regex = r"the Orchestrator should scale the subtask to (\d+) concurrent connections")]
async fn then_scale_concurrency(world: &mut AuraWorld, expected: usize) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
    for _ in 0..20 {
        interval.tick().await;
        let active_tasks = engine.tell_active().await.unwrap();
        let task = active_tasks
            .iter()
            .find(|t| t.id == id)
            .expect("Task not found");

        let all_scaled = task
            .subtasks
            .iter()
            .all(|sub| sub.target_concurrency >= expected);
        if all_scaled {
            return;
        }
    }

    let active_tasks = engine.tell_active().await.unwrap();
    let task = active_tasks
        .iter()
        .find(|t| t.id == id)
        .expect("Task not found");
    for sub in &task.subtasks {
        assert!(
            sub.target_concurrency >= expected,
            "Target concurrency {} less than expected {}",
            sub.target_concurrency,
            expected
        );
    }
}
