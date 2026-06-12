pub mod assets;
pub mod extension;
pub mod fd_limit;
pub mod jsonrpc;
pub mod launcher;
pub mod metrics;
pub mod router;
pub mod scrubber;
pub mod server;
pub mod tls;
pub mod types;
pub mod websocket;

#[cfg(test)]
pub mod test_helpers;

pub(crate) use fd_limit::adjust_file_descriptor_limit;

pub use router::create_router;
pub use types::AppState;

use aura_core::orchestrator::Engine;
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct Args {
    pub daemonize: bool,
    pub config: aura_core::AuraConfig,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub generate_tls_cert: bool,
}

pub async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    launcher::install_panic_hook();

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

    let mut config = args.config;
    adjust_file_descriptor_limit(&mut config);
    let rpc_secret = launcher::get_or_create_rpc_secret(config.network.rpc_secret.clone())?;
    let rpc_port = config.network.rpc_port;
    let bind_address = config.network.bind_address;
    let allowed_origins = config.network.allowed_origins.clone();
    let config_tls_cert = config.network.tls_cert.clone();
    let config_tls_key = config.network.tls_key.clone();

    let (engine, orchestrator, storage) = match Engine::new(config.clone()).await {
        Ok(res) => res,
        Err(e) => {
            eprintln!("Failed to initialize Aura Engine: {}", e);
            std::process::exit(1);
        }
    };
    let engine = Arc::new(engine);

    launcher::spawn_actors(orchestrator, storage);

    let metrics = Arc::new(metrics::DaemonMetrics::new());

    // Spawn background metrics updater
    let engine_metrics = Arc::clone(&engine);
    let metrics_updater = Arc::clone(&metrics);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            if let Ok(active_tasks) = engine_metrics.tell_active().await {
                let mut dl = 0.0;
                let mut ul = 0.0;
                let mut st = 0.0;

                for task in &active_tasks {
                    dl += task.completed_length as f64;
                    ul += task.uploaded_length() as f64;
                    st += task.subtasks.iter().filter(|s| s.active).count() as f64;
                }

                metrics_updater.task_count.set(active_tasks.len() as f64);
                metrics_updater.total_downloaded.set(dl);
                metrics_updater.total_uploaded.set(ul);
                metrics_updater.subtask_count.set(st);
            }
        }
    });

    let state = Arc::new(AppState {
        engine: Arc::clone(&engine),
        rpc_secret: Some(rpc_secret),
        metrics,
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    launcher::setup_signal_handler(Arc::clone(&engine), shutdown_tx).await;

    let tls_cert = args.tls_cert.or(config_tls_cert);
    let tls_key = args.tls_key.or(config_tls_key);
    let tls_config = tls::setup_tls(args.generate_tls_cert, tls_cert, tls_key)?;

    server::start_server(
        state,
        bind_address,
        rpc_port,
        allowed_origins,
        tls_config,
        config.limits.graceful_shutdown_timeout_secs,
        shutdown_rx,
    )
    .await?;

    Ok(())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
