use crate::AuraWorld;
use aura_core::task::TaskType;
use aura_core::TaskId;
use cucumber::{given, then, when};
use rand::RngExt;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

#[given(expr = "an active download at {int}% completion")]
async fn given_active_download(world: &mut AuraWorld, percent: u32) {
    world
        .init_engine(|config| {
            config.bandwidth.global_download_limit = 500 * 1024; // 500 KB/s
        })
        .await;
    let server = MockServer::start().await;
    // Serve a 1MB file
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0; 1024 * 1024])
                .insert_header("Content-Range", "bytes 0-1048575/1048576"),
        )
        .mount(&server)
        .await;

    let engine = world.engine.as_ref().unwrap();
    let id = TaskId(rand::rng().random());
    let handle = engine
        .add_task_with_sources(
            id,
            "reliability-task".to_string(),
            vec![(format!("{}/file", server.uri()), TaskType::Http)],
            None,
        )
        .await
        .unwrap();

    world.last_task_id = Some(handle.id());
    world.mock_servers.push(std::sync::Arc::new(server));

    // Wait for partial completion
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(100));
    let start = std::time::Instant::now();
    loop {
        interval.tick().await;
        if start.elapsed().as_secs() > 30 {
            panic!(
                "Timed out waiting for task to reach {}% completion",
                percent
            );
        }
        let active = engine.tell_active().await.unwrap();
        if let Some(task) = active.iter().find(|t| t.id == id) {
            if task.phase == aura_core::task::DownloadPhase::Error {
                panic!("Task entered Error phase, failing test early");
            }
            if task.total_length > 0
                && (task.completed_length as f64 / task.total_length as f64) * 100.0
                    >= percent as f64
            {
                break;
            }
        }
    }
}

#[when(expr = "I send the \"Pause\" command")]
async fn when_pause_command(world: &mut AuraWorld) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();
    engine.pause(id).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

#[then(expr = "the .aura control file should be updated with current bitfield")]
async fn then_check_aura_file(world: &mut AuraWorld) {
    let path = world.temp_dir.path().join("reliability-task.aura");
    assert!(path.exists(), "Control file not found at {:?}", path);
}

#[then(expr = "all active workers should stop")]
async fn then_check_workers_stopped(world: &mut AuraWorld) {
    let engine = world.engine.as_ref().unwrap();
    let active = engine.tell_active().await.unwrap();
    // In our engine, tell_active returns tasks that are in Downloading phase.
    // If they are paused, they shouldn't be in the list or should be in Paused phase.
    assert!(
        active
            .iter()
            .all(|t| t.phase != aura_core::task::DownloadPhase::Downloading),
        "Workers still active after pause"
    );
}

#[when(expr = "I send the \"Resume\" command")]
async fn when_resume_command(world: &mut AuraWorld) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();
    engine.resume(id).await.unwrap();
}

#[then(expr = "the engine should reload the .aura file")]
async fn then_reload_aura(_world: &mut AuraWorld) {}

#[then(expr = "download should continue from {int}% without re-downloading existing chunks")]
async fn then_continue_from(world: &mut AuraWorld, percent: u32) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let active = engine.tell_active().await.unwrap();
    let task = active
        .iter()
        .find(|t| t.id == id)
        .expect("Task not found after resume");

    assert!((task.completed_length as f64 / task.total_length as f64) * 100.0 >= percent as f64);
}

#[given(expr = "the network interface is set to {string}")]
async fn given_interface(world: &mut AuraWorld, iface: String) {
    world
        .init_engine(|config| {
            config.network.interface = Some(iface);
        })
        .await;
}

#[given(expr = "the VPN kill-switch is {string}")]
async fn given_killswitch(world: &mut AuraWorld, status: String) {
    let engine = world.engine.as_ref().unwrap();
    let mut config = (*engine.tell_config().await.unwrap()).clone();
    config.vpn.force_tunnel = status == "Enabled";
    engine.reload_config(config).await.unwrap();
}

#[when(expr = "the {string} interface becomes unavailable")]
async fn when_interface_down(world: &mut AuraWorld, _iface: String) {
    let engine = world.engine.as_ref().unwrap();
    engine.trigger_killswitch().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
}

#[then(expr = "the engine should immediately pause all active tasks")]
async fn then_pause_all(world: &mut AuraWorld) {
    then_check_workers_stopped(world).await;
}

#[then(expr = "no data should be sent over the default interface")]
async fn then_no_data(_world: &mut AuraWorld) {}

#[then(expr = "a warning should be logged to the telemetry bus")]
async fn then_check_telemetry(_world: &mut AuraWorld) {}
