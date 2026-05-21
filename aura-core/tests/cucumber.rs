use aura_core::orchestrator::{Engine, Event};
use aura_core::{Checksum, Config, TaskId};
use cucumber::World;
use tokio::sync::mpsc;
use wiremock::MockServer;

#[derive(Debug, World)]
pub struct AuraWorld {
    pub engine: Option<Engine>,
    pub last_task_id: Option<TaskId>,
    pub events_rx: Option<mpsc::UnboundedReceiver<Event>>,
    pub temp_dir: tempfile::TempDir,
    pub mirror_uris: Vec<String>,
    pub mock_servers: Vec<std::sync::Arc<MockServer>>,
    pub temp_files: Vec<tempfile::NamedTempFile>,
    pub netrc_path: Option<std::path::PathBuf>,
    pub cookie_path: Option<std::path::PathBuf>,
    pub task_checksum: Option<crate::Checksum>,
}

impl AuraWorld {
    pub async fn init_engine(&mut self, config_mod: impl FnOnce(&mut Config)) {
        let mut config = Config::default();
        let path = self.temp_dir.path().to_path_buf();
        config.storage.download_dir = path.to_str().unwrap().to_string();
        config.network.listen_port = 0; // Random port

        config_mod(&mut config);

        let (engine, orchestrator, storage) =
            Engine::new(config).await.expect("Failed to create engine");

        // Spawn engine actors
        tokio::spawn(async move {
            if let Err(e) = orchestrator.run().await {
                eprintln!("ERROR: Orchestrator failed: {}", e);
            }
        });
        tokio::spawn(async move {
            if let Err(e) = storage.run().await {
                eprintln!("ERROR: Storage failed: {}", e);
            }
        });

        let (tx, rx) = mpsc::unbounded_channel();
        let mut event_rx = engine.subscribe();

        tokio::spawn(async move {
            while let Ok(event) = event_rx.recv().await {
                let _ = tx.send(event);
            }
        });

        self.engine = Some(engine);
        self.events_rx = Some(rx);
    }
}

impl Default for AuraWorld {
    fn default() -> Self {
        Self {
            engine: None,
            last_task_id: None,
            events_rx: None,
            temp_dir: tempfile::tempdir().expect("Failed to create temp dir"),
            mirror_uris: Vec::new(),
            mock_servers: Vec::new(),
            temp_files: Vec::new(),
            netrc_path: None,
            cookie_path: None,
            task_checksum: None,
        }
    }
}

impl Drop for AuraWorld {
    fn drop(&mut self) {
        if let Some(engine) = self.engine.take() {
            tokio::spawn(async move {
                let _ = engine.shutdown().await;
            });
        }
    }
}

mod steps;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    AuraWorld::cucumber().run_and_exit("tests/features").await;
}
