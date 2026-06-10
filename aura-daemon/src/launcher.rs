use aura_core::orchestrator::Engine;
use rand::RngExt;
use std::fs;
use std::sync::Arc;
use tracing::info;

pub fn get_or_create_rpc_secret(provided_secret: Option<String>) -> Result<String, std::io::Error> {
    if let Some(secret) = aura_core::Config::resolve_rpc_secret(provided_secret) {
        return Ok(secret);
    }

    let path = aura_core::Config::rpc_secret_path();
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }

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
        "No RPC secret provided. Generated new secret and saved to {:?}.",
        path
    );
    Ok(new_secret)
}

pub fn spawn_actors(
    orchestrator: aura_core::orchestrator::Orchestrator,
    storage: aura_core::storage::StorageEngine,
) {
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
}

pub async fn setup_signal_handler(engine: Arc<Engine>, shutdown_tx: tokio::sync::mpsc::Sender<()>) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigint = signal(SignalKind::interrupt()).unwrap();
            let mut sigterm = signal(SignalKind::terminate()).unwrap();
            tokio::select! {
                _ = sigint.recv() => info!("Received SIGINT, initiating graceful shutdown"),
                _ = sigterm.recv() => info!("Received SIGTERM, initiating graceful shutdown"),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
            info!("Received Ctrl+C, initiating graceful shutdown");
        }

        let shutdown_result = tokio::select! {
            result = async {
                match tokio::time::timeout(std::time::Duration::from_secs(5), engine.shutdown()).await {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => Err(e),
                    Err(_) => {
                        tracing::warn!("Engine shutdown timed out after 5s — forcing exit");
                        Ok(())
                    }
                }
            } => result,

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
                std::process::exit(130);
            }
        };

        if let Err(e) = shutdown_result {
            tracing::error!("Shutdown failed: {}", e);
        }

        let _ = shutdown_tx.send(()).await;
    });
}

pub fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
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

        if std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&crash_path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(log_entry.as_bytes())
            })
            .is_err()
        {
            eprintln!("{}", log_entry);
        } else {
            eprintln!(
                "aura-daemon crashed. See crash report at: {}",
                crash_path.display()
            );
        }
    }));
}
