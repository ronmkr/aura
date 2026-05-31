use super::{Command, Event, Orchestrator};
use crate::dht::DhtActor;
use crate::storage::{StorageEngine, StorageEvent};
use crate::Result;
use arc_swap::ArcSwap;
use rand::RngExt;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tracing::{info, warn};

#[derive(Clone)]
pub struct Engine {
    pub(crate) command_tx: mpsc::Sender<Command>,
    pub(crate) event_tx: broadcast::Sender<Event>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine").finish()
    }
}

impl Engine {
    pub async fn new(config: crate::Config) -> Result<(Self, Orchestrator, StorageEngine)> {
        let config = Arc::new(ArcSwap::from_pointee(config));
        let (command_tx, command_rx) = mpsc::channel(100);
        let (storage_tx, storage_rx) = mpsc::channel(100);
        let (completion_tx, completion_rx) = mpsc::channel::<StorageEvent>(100);
        let (dht_tx, dht_rx) = mpsc::channel(100);
        let (nat_tx, nat_rx) = mpsc::channel(100);
        let (lpd_tx, lpd_rx) = mpsc::channel(100);

        let initial_config = config.load();
        let local_addr = {
            if let Some(addr) = initial_config.network.local_addr {
                Some(addr)
            } else if let Some(ref iface) = initial_config.network.interface {
                use local_ip_address::list_afinet_netifas;
                list_afinet_netifas()
                    .ok()
                    .and_then(|ifas: Vec<(String, std::net::IpAddr)>| {
                        ifas.into_iter()
                            .find(|(name, _)| *name == *iface)
                            .map(|(_, ip)| ip)
                    })
            } else {
                None
            }
        };

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

        let dht_actor: DhtActor = DhtActor::new(
            "0.0.0.0:6881",
            dht_id,
            dht_rx,
            local_addr,
            initial_config.network.dht_port,
            Some(storage.get_db()),
        )
        .await?;
        tokio::spawn(async move {
            if let Err(e) = dht_actor.run().await {
                warn!("DHT Actor stopped: {}", e);
            }
        });

        use crate::nat::{NatActor, NatCommand};
        let nat_actor = NatActor::new(nat_rx);
        let nat_tx_clone = nat_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = nat_actor.run().await {
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
            command_rx,
            storage_tx,
            completion_rx,
            dht_tx,
            lpd_tx.clone(),
            nat_tx,
            config.clone(),
            storage.get_db(),
            dns_resolver,
        );

        use crate::lpd::LpdActor;
        if initial_config.bittorrent.lpd_enabled {
            let lpd_subtask_tx = orchestrator.subtask_tx.clone();
            let lpd_actor = LpdActor::new(lpd_rx, lpd_subtask_tx, local_addr).await?;
            tokio::spawn(async move {
                if let Err(e) = lpd_actor.run().await {
                    warn!("LPD Actor stopped: {}", e);
                }
            });
        }

        // Setup config file watcher
        let config_path = std::path::PathBuf::from("Aura.toml");
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
                    if let Ok(new_config) = crate::Config::from_file(&config_path_watcher) {
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

        Ok((
            Self {
                command_tx,
                event_tx,
            },
            orchestrator,
            storage,
        ))
    }
}
