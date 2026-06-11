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
    pub resolved_config: Option<Config>,
    pub original_cwd: Option<std::path::PathBuf>,
    pub original_home: Option<std::ffi::OsString>,
}

impl AuraWorld {
    pub async fn init_engine(&mut self, config_mod: impl FnOnce(&mut Config)) {
        let mut config = Config::default();
        let path = self.temp_dir.path().to_path_buf();
        config.storage.download_dir = path.to_str().unwrap().to_string();
        config.network.listen_port = 0; // Random port
        config.network.http_retry_count = 0; // Disable retries to speed up tests
        config.network.connect_timeout_secs = 1; // Fast timeout for tests

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
            resolved_config: None,
            original_cwd: None,
            original_home: None,
        }
    }
}

impl Drop for AuraWorld {
    fn drop(&mut self) {
        if let Some(ref cwd) = self.original_cwd {
            let _ = std::env::set_current_dir(cwd);
        }
        if let Some(ref home) = self.original_home {
            std::env::set_var("HOME", home);
        } else if self.original_home.is_none() {
            // Note: std::env::remove_var is not safe in multi-threaded contexts, but here scenarios run sequentially
            std::env::remove_var("HOME");
        }
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
    AuraWorld::cucumber()
        .max_concurrent_scenarios(1)
        .filter_run_and_exit("tests/features", |_, _, _sc| {
            #[cfg(not(feature = "s3"))]
            if _sc.tags.iter().any(|t| t == "s3") {
                return false;
            }
            #[cfg(not(feature = "gdrive"))]
            if _sc.tags.iter().any(|t| t == "gdrive") {
                return false;
            }
            true
        })
        .await;
}
