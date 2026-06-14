use super::{Command, Event, Orchestrator};
use crate::dht::DhtManager;
use crate::storage::{StorageEngine, StorageEvent};
use crate::Result;
use arc_swap::ArcSwap;
use rand::RngExt;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct Engine {
    pub(crate) command_tx: mpsc::Sender<Command>,
    pub(crate) event_tx: broadcast::Sender<Event>,
    pub config: Arc<ArcSwap<crate::AuraConfig>>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine").finish()
    }
}

impl super::traits::EventSubscriber for Engine {
    fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }
}

impl Engine {
    pub async fn new(config: crate::AuraConfig) -> Result<(Self, Orchestrator, StorageEngine)> {
        let (command_tx, command_rx) = mpsc::channel(config.limits.command_channel_capacity);
        let (storage_tx, storage_rx) = mpsc::channel(config.limits.storage_channel_capacity);
        let (completion_tx, completion_rx) =
            mpsc::channel::<StorageEvent>(config.limits.storage_channel_capacity);
        let (nat_tx, nat_rx) = mpsc::channel(config.limits.command_channel_capacity);
        let (lpd_tx, lpd_rx) = mpsc::channel(config.limits.command_channel_capacity);

        let config = Arc::new(ArcSwap::from_pointee(config));

        let initial_config = config.load();
        let local_addr = initial_config.resolve_local_addr();

        let db_path = std::path::PathBuf::from(&initial_config.storage.download_dir)
            .join(".aura")
            .join("metadata.db");
        let storage = StorageEngine::new(
            storage_rx,
            completion_tx,
            Some(db_path),
            Some(config.clone()),
        );

        let mut dht_id = [0u8; 20];
        rand::rng().fill(&mut dht_id);

        let dht_bind_addr = format!(
            "{}:{}",
            initial_config.network.bind_address, initial_config.network.dht_port
        );
        let dht_manager = DhtManager::new(
            &dht_bind_addr,
            dht_id,
            local_addr,
            initial_config.network.dht_port,
            Some(storage.db.clone()),
            config.clone(),
            initial_config.limits.command_channel_capacity,
        )
        .await?;
        let dht_tx = dht_manager.tx;

        use crate::nat::{NatActor, NatCommand};
        let nat_actor = NatActor::new(nat_rx);
        let nat_tx_clone = nat_tx.clone();
        let nat_config_clone = config.clone();
        tokio::spawn(async move {
            if let Err(e) = nat_actor.run(nat_config_clone).await {
                warn!("NAT Actor stopped: {}", e);
            }
        });

        // Request initial port mapping
        let _ = nat_tx_clone
            .send(NatCommand::MapPort {
                port: initial_config.network.listen_port,
                description: "Aura BitTorrent".to_string(),
            })
            .await;

        let dns_resolver = crate::net_util::create_resolver(&initial_config.network.dns_resolver)
            .await
            .unwrap_or_else(|_| {
                hickory_resolver::TokioResolver::builder_tokio()
                    .unwrap()
                    .build()
                    .unwrap()
            });
        let dns_resolver = Arc::new(dns_resolver);

        let (orchestrator, event_tx) = Orchestrator::new(
            crate::orchestrator::state::OrchestratorChannels {
                command_rx,
                storage_client: Arc::new(crate::storage::StorageClient::new(storage_tx)),
                storage_completion_rx: completion_rx,
                dht_tx,
                lpd_tx,
                nat_tx,
            },
            config.clone(),
            storage.get_db(),
            dns_resolver,
        );

        use crate::lpd::LpdActor;
        if initial_config.bittorrent.lpd_enabled {
            let lpd_subtask_tx = orchestrator.subtask_tx.clone();
            let lpd_actor = LpdActor::new(lpd_rx, lpd_subtask_tx.clone(), local_addr).await?;
            let lpd_config_clone = config.clone();
            tokio::spawn(async move {
                if let Err(e) = lpd_actor.run(lpd_config_clone).await {
                    warn!("LPD Actor stopped: {}", e);
                }
            });
        }

        // Setup config file watcher
        if let Some(ref config_path) = initial_config.config_path {
            if config_path.exists() {
                let command_tx_watcher = command_tx.clone();
                let config_path_watcher = config_path.clone();

                tokio::spawn(async move {
                    use notify::{EventKind, RecursiveMode, Watcher};
                    let (tx, mut rx) = mpsc::channel(1);

                    let mut watcher =
                        notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                            if let Ok(event) = res {
                                if matches!(event.kind, EventKind::Modify(_)) {
                                    let _ = tx.blocking_send(());
                                }
                            }
                        })
                        .expect("Failed to create config watcher");

                    watcher
                        .watch(&config_path_watcher, RecursiveMode::NonRecursive)
                        .expect("Failed to watch config");

                    while rx.recv().await.is_some() {
                        info!("Config file modified, reloading...");
                        if let Ok(new_config) = crate::AuraConfig::from_file(&config_path_watcher) {
                            let (tx, _rx) = tokio::sync::oneshot::channel();
                            let _ = command_tx_watcher
                                .send(Command::ReloadConfig(Arc::new(new_config), tx))
                                .await;
                        } else {
                            warn!("Failed to reload modified config");
                        }
                    }

                    // Keep watcher alive as long as loop runs
                    drop(watcher);
                });
            }
        }

        // Setup watch directory watcher
        if let Some(ref watch_dir_str) = initial_config.storage.watch_dir {
            let watch_dir_path = std::path::PathBuf::from(watch_dir_str);
            if watch_dir_path.exists() {
                let command_tx_watcher = command_tx.clone();
                let config_watcher = config.clone();
                let event_tx_watcher = event_tx.clone();

                tokio::spawn(async move {
                    use notify::{EventKind, RecursiveMode, Watcher};
                    use std::time::Duration;

                    let (tx, mut rx) = mpsc::channel(100);
                    let mut watcher =
                        notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                            if let Ok(event) = res {
                                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_))
                                {
                                    for path in event.paths {
                                        let _ = tx.blocking_send(path);
                                    }
                                }
                            }
                        })
                        .expect("Failed to create watch directory watcher");

                    watcher
                        .watch(&watch_dir_path, RecursiveMode::NonRecursive)
                        .expect("Failed to watch watch directory");

                    info!("Watch folder monitoring enabled at {:?}", watch_dir_path);

                    let engine_instance = Self {
                        command_tx: command_tx_watcher,
                        event_tx: event_tx_watcher,
                        config: config_watcher,
                    };

                    while let Some(path) = rx.recv().await {
                        // Skip if it's already in processed or failed folders
                        if let Some(parent) = path.parent() {
                            if parent.ends_with("processed") || parent.ends_with("failed") {
                                continue;
                            }
                        }

                        // Debounce logic: wait 1 second for write completion
                        tokio::time::sleep(Duration::from_secs(1)).await;

                        if !path.exists() {
                            continue;
                        }

                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_lowercase();

                        if ext != "torrent" && ext != "metalink" && ext != "meta4" && ext != "nzb" {
                            continue;
                        }

                        info!("Watch folder: detected file {:?}", path);

                        let result =
                            crate::orchestrator::watch::ingest_watch_file(&engine_instance, &path)
                                .await;

                        let file_name = match path.file_name() {
                            Some(f) => f,
                            None => continue,
                        };

                        let mut move_or_delete_failed = false;
                        let mut final_err = None;

                        if result.is_ok() {
                            let dest_dir = watch_dir_path.join("processed");
                            let _ = std::fs::create_dir_all(&dest_dir);
                            let dest_path = dest_dir.join(file_name);
                            if let Err(e) = std::fs::rename(&path, &dest_path) {
                                warn!(
                                    "Watch folder: failed to move {:?} to processed folder: {}",
                                    path, e
                                );
                                if let Err(e2) = std::fs::remove_file(&path) {
                                    move_or_delete_failed = true;
                                    final_err = Some(e2);
                                }
                            } else {
                                info!("Watch folder: moved {:?} to processed folder", file_name);
                            }
                        } else {
                            let err_msg = result.err().unwrap();
                            warn!(
                                "Watch folder ingestion failed for {:?}: {}",
                                file_name, err_msg
                            );
                            let dest_dir = watch_dir_path.join("failed");
                            let _ = std::fs::create_dir_all(&dest_dir);
                            let dest_path = dest_dir.join(file_name);
                            if let Err(e) = std::fs::rename(&path, &dest_path) {
                                warn!(
                                    "Watch folder: failed to move {:?} to failed folder: {}",
                                    path, e
                                );
                                if let Err(e2) = std::fs::remove_file(&path) {
                                    move_or_delete_failed = true;
                                    final_err = Some(e2);
                                }
                            }
                        }

                        if move_or_delete_failed {
                            error!(
                                "Watch folder: permission error. Cannot move or delete file {:?}. \
                                Disabling watch folder watcher to prevent infinite loops. Error: {:?}",
                                path, final_err
                            );
                            break;
                        }
                    }

                    drop(watcher);
                });
            }
        }

        Ok((
            Self {
                command_tx,
                event_tx,
                config,
            },
            orchestrator,
            storage,
        ))
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
