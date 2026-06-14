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

use aura_core::orchestrator::{Engine, TaskQuerier};
use std::sync::Arc;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub struct Args {
    pub daemonize: bool,
    pub config: aura_core::AuraConfig,
    pub tls_cert: Option<String>,
    pub tls_key: Option<String>,
    pub generate_tls_cert: bool,
    pub custom_shutdown: Option<tokio::sync::mpsc::Receiver<()>>,
}

pub async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        daemonize: _,
        mut config,
        tls_cert,
        tls_key,
        generate_tls_cert,
        custom_shutdown,
    } = args;

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

    // Spawn background RSS feed poller
    let (rss_refresh_tx, mut rss_refresh_rx) = tokio::sync::mpsc::channel::<()>(10);
    let engine_rss = Arc::clone(&engine);
    tokio::spawn(async move {
        let mut last_poll_times: std::collections::HashMap<String, std::time::Instant> =
            std::collections::HashMap::new();
        let mut failure_counts: std::collections::HashMap<String, u32> =
            std::collections::HashMap::new();
        let rss_manager = aura_core::rss::RssManager::new();

        loop {
            if let Ok(subs) = rss_manager.load_subscriptions() {
                let client = reqwest::Client::new();
                for sub in subs {
                    let interval_mins = sub.poll_interval.unwrap_or(30);
                    let failures = failure_counts.get(&sub.url).copied().unwrap_or(0);
                    let backoff_factor = 2u64.pow(failures.min(6)); // Cap at 64x
                    let effective_interval = interval_mins * 60 * backoff_factor;

                    let should_poll = match last_poll_times.get(&sub.url) {
                        Some(last_time) => {
                            last_time.elapsed()
                                >= std::time::Duration::from_secs(effective_interval)
                        }
                        None => true,
                    };

                    if should_poll {
                        tracing::info!("Polling RSS feed '{}' ({})", sub.name, sub.url);
                        last_poll_times.insert(sub.url.clone(), std::time::Instant::now());

                        match client
                            .get(&sub.url)
                            .timeout(std::time::Duration::from_secs(30))
                            .send()
                            .await
                        {
                            Ok(resp) => {
                                if resp.status().is_success() {
                                    failure_counts.insert(sub.url.clone(), 0);
                                    if let Ok(content) = resp.bytes().await {
                                        match aura_core::rss::parse_feed(&content[..]) {
                                            Ok(items) => {
                                                for item in items {
                                                    if rss_manager.is_ingested(&item.guid) {
                                                        continue;
                                                    }
                                                    if aura_core::rss::RssManager::matches_filters(
                                                        &item.title,
                                                        item.category.as_deref(),
                                                        item.size,
                                                        &sub.filters,
                                                        &sub.categories,
                                                        sub.max_size,
                                                    ) {
                                                        tracing::info!("RSS Match: Ingesting task '{}' from URL '{}'", item.title, item.link);

                                                        let task_id = aura_core::TaskId::random();
                                                        let ttype = if let Some(detected) =
                                                            aura_core::orchestrator::protocol_detector::ProtocolDetector::detect(&item.link).await
                                                        {
                                                            detected.to_task_type()
                                                        } else {
                                                            aura_core::task::TaskType::Http
                                                        };

                                                        let args = aura_core::orchestrator::command::AddTaskArgs {
                                                            id: task_id,
                                                            tenant_id: None,
                                                            name: item.title.clone(),
                                                            sources: vec![(item.link.clone(), ttype)],
                                                            checksum: None,
                                                            priority: 3,
                                                            streaming_mode: false,
                                                            depends_on: Vec::new(),
                                                            follow_on: None,
                                                        };

                                                        match engine_rss
                                                            .add_task_with_options(args)
                                                            .await
                                                        {
                                                            Ok(_) => {
                                                                let _ = rss_manager
                                                                    .mark_ingested(&item.guid);
                                                            }
                                                            Err(
                                                                aura_core::Error::DuplicateTask(_),
                                                            ) => {
                                                                let _ = rss_manager
                                                                    .mark_ingested(&item.guid);
                                                            }
                                                            Err(e) => {
                                                                tracing::error!("RSS Ingestion failed for '{}': {}", item.title, e);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Error parsing RSS feed '{}': {}",
                                                    sub.name,
                                                    e
                                                );
                                            }
                                        }
                                    }
                                } else {
                                    tracing::error!(
                                        "Feed '{}' returned HTTP error: {}",
                                        sub.name,
                                        resp.status()
                                    );
                                    let entry = failure_counts.entry(sub.url.clone()).or_insert(0);
                                    *entry = entry.saturating_add(1);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to fetch RSS feed '{}': {}", sub.name, e);
                                let entry = failure_counts.entry(sub.url.clone()).or_insert(0);
                                *entry = entry.saturating_add(1);
                            }
                        }
                    }
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {}
                _ = rss_refresh_rx.recv() => {
                    tracing::info!("Manual RSS refresh triggered via JSON-RPC");
                    last_poll_times.clear();
                    failure_counts.clear();
                }
            }
        }
    });

    let state = Arc::new(AppState {
        engine: Arc::clone(&engine),
        rpc_secret: Some(rpc_secret),
        metrics,
        rss_refresh_tx: Some(rss_refresh_tx),
    });

    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);
    launcher::setup_signal_handler(Arc::clone(&engine), shutdown_tx, custom_shutdown).await;

    let tls_cert = tls_cert.or(config_tls_cert);
    let tls_key = tls_key.or(config_tls_key);
    let tls_config = tls::setup_tls(generate_tls_cert, tls_cert, tls_key)?;

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
