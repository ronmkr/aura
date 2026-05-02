use std::sync::Arc;
use tokio::sync::{mpsc, broadcast};
use tracing::{warn, info};
use arc_swap::ArcSwap;
use rand::Rng;
use crate::{Result, TaskId, Error};
use crate::task::{MetaTask, TaskType};
use crate::storage::{StorageEngine};
use super::{Orchestrator, Command, Event};

pub struct Engine {
    pub(crate) command_tx: mpsc::Sender<Command>,
    pub(crate) event_rx: broadcast::Receiver<Event>,
}

impl Engine {
    pub async fn new(config: crate::Config) -> Result<(Self, Orchestrator, StorageEngine)> {
        let config = Arc::new(ArcSwap::from_pointee(config));
        let (command_tx, command_rx) = mpsc::channel(100);
        let (storage_tx, storage_rx) = mpsc::channel(100);
        let (completion_tx, completion_rx) = mpsc::channel(100);
        let (dht_tx, dht_rx) = mpsc::channel(100);
        let (nat_tx, nat_rx) = mpsc::channel(100);
        
        let initial_config = config.load();
        let local_addr = {
            if let Some(addr) = initial_config.network.local_addr {
                Some(addr)
            } else if let Some(ref iface) = initial_config.network.interface {
                use local_ip_address::list_afinet_netifas;
                list_afinet_netifas().ok().and_then(|ifas: Vec<(String, std::net::IpAddr)>| {
                    ifas.into_iter().find(|(name, _)| *name == *iface).map(|(_, ip)| ip)
                })
            } else {
                None
            }
        };

        use crate::dht::DhtActor;
        let mut dht_id = [0u8; 20];
        rand::thread_rng().fill(&mut dht_id);
        
        let dht_actor = DhtActor::new("0.0.0.0:6881", dht_id, dht_rx, local_addr, initial_config.network.dht_port).await?;
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
        let _ = nat_tx_clone.send(NatCommand::MapPort {
            port: initial_config.network.listen_port,
            description: "Aura BitTorrent".to_string(),
        }).await;

        let storage = StorageEngine::new(storage_rx, completion_tx);
        let (orchestrator, event_rx) = Orchestrator::new(command_rx, storage_tx, completion_rx, dht_tx, nat_tx, config.clone());
        
        // Setup config file watcher
        let config_path = std::path::PathBuf::from("Aura.toml");
        if config_path.exists() {
            let command_tx_watcher = command_tx.clone();
            let config_path_watcher = config_path.clone();
            
            tokio::spawn(async move {
                use notify::{Watcher, RecursiveMode, EventKind};
                let (tx, mut rx) = mpsc::channel(1);
                
                let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                    if let Ok(event) = res {
                        if matches!(event.kind, EventKind::Modify(_)) {
                            let _ = tx.blocking_send(());
                        }
                    }
                }).expect("Failed to create config watcher");
                
                watcher.watch(&config_path_watcher, RecursiveMode::NonRecursive).expect("Failed to watch config");
                
                while rx.recv().await.is_some() {
                    info!("Config file modified, reloading...");
                    if let Ok(new_config) = crate::Config::from_file(&config_path_watcher) {
                        let _ = command_tx_watcher.send(Command::ReloadConfig(Arc::new(new_config))).await;
                    } else {
                        warn!("Failed to reload modified config");
                    }
                }
                
                // Keep watcher alive as long as loop runs
                drop(watcher);
            });
        }

        Ok((
            Self { command_tx, event_rx },
            orchestrator,
            storage,
        ))
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.event_rx.resubscribe()
    }

    pub async fn add_task(&self, name: String, uri: String, _length: u64, task_type: TaskType) -> Result<TaskId> {
        let id = TaskId(rand::random());
        self.add_task_with_sources(id, name, vec![(uri, task_type)]).await
    }

    pub async fn add_task_with_id(&self, id: TaskId, name: String, uri: String, _length: u64, task_type: TaskType) -> Result<TaskId> {
        self.add_task_with_sources(id, name, vec![(uri, task_type)]).await
    }

    pub async fn add_task_with_sources(&self, id: TaskId, name: String, sources: Vec<(String, TaskType)>) -> Result<TaskId> {
        self.command_tx.send(Command::AddTask { id, name, sources }).await
            .map_err(|e| Error::Storage(e.to_string()))?;
        Ok(id)
    }

    pub async fn tell_active(&self) -> Result<Vec<MetaTask>> {
        let (tx, mut rx) = mpsc::channel(1);
        self.command_tx.send(Command::ListActive(tx)).await
            .map_err(|e| Error::Storage(e.to_string()))?;
        rx.recv().await.ok_or_else(|| Error::Storage("Engine shut down".to_string()))
    }

    pub async fn pause(&self, id: TaskId) -> Result<()> {
        self.command_tx.send(Command::Pause(id)).await
            .map_err(|e| Error::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn unpause(&self, id: TaskId) -> Result<()> {
        self.command_tx.send(Command::Resume(id)).await
            .map_err(|e| Error::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn load_tasks_from_dir(&self, dir: &str) -> Result<()> {
        let mut entries = tokio::fs::read_dir(dir).await
            .map_err(|e| Error::Storage(format!("Failed to read dir: {}", e)))?;
        
        while let Some(entry) = entries.next_entry().await.map_err(|e| Error::Storage(e.to_string()))? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("aura") {
                let data = tokio::fs::read(&path).await
                    .map_err(|e| Error::Storage(e.to_string()))?;
                let state: crate::task::TaskState = serde_json::from_slice(&data)
                    .map_err(|e| Error::Storage(e.to_string()))?;
                
                info!("Found persisted task: {}", state.name);
                // For now we just add it back with original sources
                let sources = state.subtasks.iter().map(|s| (s.uri.clone(), s.task_type.clone())).collect();
                let id = state.id;
                let _ = self.add_task_with_sources(id, state.name, sources).await;
            }
        }
        Ok(())
    }

    pub async fn remove(&self, id: TaskId) -> Result<()> {
        self.command_tx.send(Command::Remove(id)).await
            .map_err(|e| Error::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<()> {
        self.command_tx.send(Command::Shutdown).await
            .map_err(|e| Error::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn reload_config(&self, config: crate::Config) -> Result<()> {
        self.command_tx.send(Command::ReloadConfig(Arc::new(config))).await
            .map_err(|e| Error::Config(e.to_string()))?;
        Ok(())
    }
}
