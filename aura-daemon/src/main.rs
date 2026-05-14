use aura_core::orchestrator::Engine;
use aura_daemon::{create_router, AppState};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    info!("Starting Aura Daemon");

    // Bootstrap the engine
    let config = aura_core::Config::from_file("Aura.toml").unwrap_or_default();
    let rpc_secret = config.network.rpc_secret.clone();

    let (engine, orchestrator, storage) = match Engine::new(config).await {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Failed to initialize Aura Engine: {}", e);
            std::process::exit(1);
        }
    };
    let engine = Arc::new(engine);

    // Spawn the actors
    tokio::spawn(async move {
        if let Err(e) = orchestrator.run().await {
            eprintln!("Orchestrator error: {}", e);
        }
    });

    tokio::spawn(async move {
        if let Err(e) = storage.run().await {
            eprintln!("Storage Engine error: {}", e);
        }
    });

    let state = Arc::new(AppState { engine, rpc_secret });

    let app = create_router(state).layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:6800").await?;
    info!("RPC Server listening on http://0.0.0.0:6800");
    axum::serve(listener, app).await?;

    Ok(())
}
