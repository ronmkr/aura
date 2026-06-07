pub mod assets;
pub mod extension;
pub mod fd_limit;
pub mod jsonrpc;
pub mod metrics;
pub mod router;
pub mod scrubber;
pub mod tls;
pub mod types;
pub mod websocket;

pub(crate) use fd_limit::adjust_file_descriptor_limit;

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
    pub daemonize: bool,
    pub config: aura_core::Config,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub generate_tls_cert: bool,
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
        "No RPC secret provided. Generated new secret and saved to {:?}. \
         Copy it from that file to authenticate RPC calls.",
        path
    );
    // SECURITY: Never log the secret value — it is visible in log aggregators
    // and the system journal. Users must read it from the file directly. (#239)

    Ok(new_secret)
}

/// Installs a global panic hook that writes a crash report to `~/.aura/crash.log`
/// before the process exits. This ensures panics in any Tokio task produce a
/// diagnosable crash record rather than a silent exit. (Issue #246, ADR-0064)
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        // Determine crash log path
        let crash_path = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".aura")
            .join("crash.log");

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_string());

        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("<non-string panic payload>");

        let log_entry = format!(
            "=== AURA DAEMON CRASH REPORT ===\n\
             Timestamp : {}\n\
             Location  : {}\n\
             Message   : {}\n\
             ================================\n",
            timestamp, location, payload
        );

        // Write to crash.log; if that fails, fall back to stderr
        let written = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(log_entry.as_bytes())
            });

        if written.is_err() {
            // Last-resort: write to stderr
            eprintln!("{}", log_entry);
        } else {
            eprintln!(
                "aura-daemon crashed. See crash report at: {}",
                crash_path.display()
            );
        }
    }));
}

pub async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    install_panic_hook();

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
    let mut config = args.config;
    adjust_file_descriptor_limit(&mut config);
    let rpc_secret = get_or_create_rpc_secret(config.network.rpc_secret.clone())?;
    let rpc_port = config.network.rpc_port;
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
                    ul += task.uploaded_length as f64;
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

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    let engine_clone = Arc::clone(&engine);
    tokio::spawn(async move {
        // ---- First signal: begin graceful shutdown ----
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigint = signal(SignalKind::interrupt()).unwrap();
            let mut sigterm = signal(SignalKind::terminate()).unwrap();
            tokio::select! {
                _ = sigint.recv() => info!("Received SIGINT, initiating graceful shutdown (send again to force quit)"),
                _ = sigterm.recv() => info!("Received SIGTERM, initiating graceful shutdown (send again to force quit)"),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            info!("Received Ctrl+C, initiating graceful shutdown (press again to force quit)");
        }

        // Race: 5-second graceful shutdown vs. second signal (force quit)
        // ADR-0058: 5-second timeout; second signal causes immediate exit.
        let shutdown_result = tokio::select! {
            result = async {
                match tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    engine_clone.shutdown(),
                ).await {
                    Ok(Ok(())) => {
                        info!("Engine shutdown completed successfully");
                        Ok(())
                    }
                    Ok(Err(e)) => {
                        tracing::error!("Engine shutdown error: {}", e);
                        Err(e)
                    }
                    Err(_) => {
                        tracing::warn!("Engine shutdown timed out after 5s — forcing exit");
                        Ok(())
                    }
                }
            } => result,

            // Second signal: force-quit immediately
            _ = async {
                #[cfg(unix)]
                {
                    use tokio::signal::unix::{signal, SignalKind};
                    let mut sigint2 = signal(SignalKind::interrupt()).unwrap();
                    let mut sigterm2 = signal(SignalKind::terminate()).unwrap();
                    tokio::select! {
                        _ = sigint2.recv() => {}
                        _ = sigterm2.recv() => {}
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = tokio::signal::ctrl_c().await;
                }
            } => {
                tracing::warn!("Second shutdown signal received — forcing immediate exit");
                std::process::exit(130); // SIGINT exit code convention
            }
        };

        if let Err(e) = shutdown_result {
            tracing::error!("Shutdown failed: {}", e);
        }

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
                // moz-extension:// removed — Chrome-only per ADR-0049
            },
        ))
        .allow_methods([axum::http::Method::GET, axum::http::Method::POST])
        .allow_headers(tower_http::cors::Any);

    let app = create_router(state).layer(cors);

    // Restrict bind from 0.0.0.0 to 127.0.0.1
    let addr = format!("127.0.0.1:{}", rpc_port);

    let tls_cert = args.tls_cert.or(config_tls_cert);
    let tls_key = args.tls_key.or(config_tls_key);

    if let Some((cert_path, key_path)) = tls::setup_tls(args.generate_tls_cert, tls_cert, tls_key)?
    {
        let rustls_config =
            axum_server::tls_rustls::RustlsConfig::from_pem_file(cert_path, key_path).await?;
        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();

        let shutdown_timeout = config.limits.graceful_shutdown_timeout_secs;
        tokio::spawn(async move {
            let _ = shutdown_rx.recv().await;
            info!("RPC server stopping (HTTPS)");
            shutdown_handle
                .graceful_shutdown(Some(std::time::Duration::from_secs(shutdown_timeout)));
        });

        info!("RPC Server listening (HTTPS) on https://{}", addr);
        axum_server::bind_rustls(addr.parse::<std::net::SocketAddr>()?, rustls_config)
            .handle(handle)
            .serve(app.into_make_service())
            .await?;
    } else {
        let listener = tokio::net::TcpListener::bind(&addr).await?;
        info!("RPC Server listening on http://{}", addr);

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.recv().await;
                info!("RPC server stopped");
            })
            .await?;
    }

    Ok(())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
