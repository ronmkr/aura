pub mod assets;
pub mod extension;
pub mod jsonrpc;
pub mod metrics;
pub mod router;
pub mod scrubber;
pub mod types;
pub mod websocket;

pub use router::create_router;
pub use types::AppState;

use aura_core::orchestrator::Engine;
use rand::RngExt;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct Args {
    pub rpc_port: u16,
    pub rpc_secret: Option<String>,
    pub daemonize: bool,
    pub config: Option<String>,
}

fn get_or_create_rpc_secret(provided_secret: Option<String>) -> Result<String, std::io::Error> {
    if let Some(secret) = provided_secret {
        return Ok(secret);
    }

    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);

    let mut path = match home {
        Some(h) => h,
        None => PathBuf::from("."),
    };
    path.push(".aura");
    if !path.exists() {
        fs::create_dir_all(&path)?;
    }
    path.push("rpc_secret");

    if path.exists() {
        let secret = fs::read_to_string(&path)?;
        let trimmed = secret.trim().to_string();
        if !trimmed.is_empty() {
            return Ok(trimmed);
        }
    }

    // Generate new secret (32 alphanumeric characters)
    let new_secret: String = rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        use std::io::Write;
        file.write_all(new_secret.as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        fs::write(&path, &new_secret)?;
    }

    info!(
        "No RPC secret provided. Generated new token and saved to {:?}",
        path
    );
    info!("RPC Secret: {}", new_secret);

    Ok(new_secret)
}

pub async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Setup JSON tracing for audit logs if not already set
    let _ = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(crate::scrubber::ScrubbingMakeWriter::new(std::io::stdout)),
        )
        .try_init();

    info!("Starting Aura Daemon");

    // Bootstrap the engine
    let config = aura_core::Config::from_file("Aura.toml").unwrap_or_default();
    let rpc_secret = get_or_create_rpc_secret(args.rpc_secret)?;
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
        engine: Arc::clone(&engine),
        rpc_secret: Some(rpc_secret),
    });

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let engine_clone = Arc::clone(&engine);
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigint = signal(SignalKind::interrupt()).unwrap();
            let mut sigterm = signal(SignalKind::terminate()).unwrap();

            tokio::select! {
                _ = sigint.recv() => {
                    info!("Received SIGINT, initiating graceful shutdown");
                }
                _ = sigterm.recv() => {
                    info!("Received SIGTERM, initiating graceful shutdown");
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            info!("Received Ctrl+C, initiating graceful shutdown");
        }

        if let Err(e) = engine_clone.shutdown().await {
            tracing::error!("Failed to shut down engine gracefully: {}", e);
        } else {
            info!("Engine shutdown completed successfully");
        }

        // Wait a short grace period for in-flight flushes (ADR 0058 timeout)
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let _ = shutdown_tx.send(()).await;
    });

    // Replace permissive CORS with localhost/extension restriction
    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::AllowOrigin::predicate(
            |origin, _parts| {
                let origin_bytes = origin.as_bytes();
                origin_bytes.starts_with(b"http://localhost")
                    || origin_bytes.starts_with(b"http://127.0.0.1")
                    || origin_bytes.starts_with(b"chrome-extension://")
                    || origin_bytes.starts_with(b"moz-extension://")
            },
        ))
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(tower_http::cors::Any);

    let app = create_router(state).layer(cors);

    // Restrict bind from 0.0.0.0 to 127.0.0.1
    let addr = format!("127.0.0.1:{}", rpc_port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("RPC Server listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.recv().await;
            info!("RPC server stopped");
        })
        .await?;

    Ok(())
}
