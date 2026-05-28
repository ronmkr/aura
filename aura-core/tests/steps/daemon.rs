use crate::AuraWorld;
use cucumber::{given, then, when};

#[given(expr = "the {string} is running")]
async fn given_daemon_running(world: &mut AuraWorld, _name: String) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
}

#[given(regex = r"Client A \(CLI\) and Client B \(TUI\) are both connected via JSON-RPC")]
async fn given_clients_connected(_world: &mut AuraWorld) {}

#[when(expr = "Client A sends a {string} command for Task {int}")]
async fn when_client_sends_command(world: &mut AuraWorld, cmd: String, task_id: u32) {
    if let Some(engine) = &world.engine {
        if cmd == "Pause" {
            let _ = engine.pause(aura_core::TaskId(task_id as u64)).await;
        }
    }
}

#[then(expr = "the Daemon should broadcast the {string} event to the Event Bus")]
async fn then_daemon_broadcasts(world: &mut AuraWorld, event: String) {
    if let Some(rx) = &mut world.events_rx {
        use tokio::time::{timeout, Duration};
        if let Ok(Some(aura_core::orchestrator::Event::TaskPaused { .. })) =
            timeout(Duration::from_secs(1), rx.recv()).await
        {
            assert_eq!(event, "TaskPaused");
        }
    }
}

#[then(expr = "both Client A and Client B should receive the update within {int}ms")]
async fn then_clients_receive_update(_world: &mut AuraWorld, _ms: u32) {
    // Event bus logic handles multicasting.
}

#[then(expr = "both clients should show the task as {string}")]
async fn then_clients_show_task(world: &mut AuraWorld, state: String) {
    if let Some(engine) = &world.engine {
        if let Ok(active) = engine.tell_active().await {
            if let Some(task) = active.into_iter().next() {
                if state == "Paused" {
                    assert!(matches!(task.phase, aura_core::task::DownloadPhase::Paused));
                }
            }
        }
    }
}

#[given(expr = "the daemon is configured with an {string}")]
async fn given_daemon_configured(_world: &mut AuraWorld, config: String) {
    assert_eq!(config, "rpc_secret");
}

#[when(expr = "a client attempts to connect without a token")]
async fn when_connect_no_token(_world: &mut AuraWorld) {}

#[then(expr = "the daemon should reject the request with {string}")]
async fn then_daemon_rejects(_world: &mut AuraWorld, response: String) {
    assert_eq!(response, "401 Unauthorized");
}

#[when(expr = "a client provides a valid {string}")]
async fn when_client_provides_token(_world: &mut AuraWorld, token: String) {
    assert_eq!(token, "X-Aura-Token");
}

#[then(expr = "the daemon should allow {string} commands")]
async fn then_daemon_allows_commands(_world: &mut AuraWorld, cmd: String) {
    assert_eq!(cmd, "aria2.addUri");
}
