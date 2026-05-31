pub mod assets;
pub mod extension;
pub mod jsonrpc;
pub mod metrics;
pub mod router;
pub mod types;
pub mod websocket;

pub use router::create_router;
pub use types::AppState;

use aura_core::orchestrator::Engine;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct Args {
    pub rpc_port: u16,
    pub rpc_secret: String,
    pub daemonize: bool,
    pub config: Option<String>,
}

pub async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Setup JSON tracing for audit logs if not already set
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer().json())
        .try_init();

    info!("Starting Aura Daemon");

    // Bootstrap the engine
    let config = aura_core::Config::from_file("Aura.toml").unwrap_or_default();
    let rpc_secret = args.rpc_secret;
    let rpc_port = args.rpc_port;

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

    let state = Arc::new(AppState {
        engine,
        rpc_secret: Some(rpc_secret),
    });

    let app = create_router(state).layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{}", rpc_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("RPC Server listening on http://{}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
