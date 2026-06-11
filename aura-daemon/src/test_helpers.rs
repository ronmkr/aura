use crate::types::AppState;
use aura_core::orchestrator::Engine;
use aura_core::Config;
use std::sync::Arc;

pub async fn setup_test_app(secret: Option<String>) -> (axum::Router, tempfile::TempDir) {
    let mut config = Config::default();
    config.network.dht_port = 0;
    let temp_dir = tempfile::tempdir().unwrap();
    config.storage.download_dir = temp_dir.path().to_string_lossy().into_owned();

    let (engine, _orchestrator, _storage) = Engine::new(config).await.unwrap();
    let metrics = Arc::new(crate::metrics::DaemonMetrics::new());
    let state = Arc::new(AppState {
        engine: Arc::new(engine),
        rpc_secret: secret,
        metrics,
    });
    (crate::router::create_router(state), temp_dir)
}
