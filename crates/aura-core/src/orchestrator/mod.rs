use crate::bitfield::Bitfield;
use crate::bt_task::BtTask;
use crate::bt_worker::PeerId;
use crate::dht::DhtCommand;
use crate::nat::NatCommand;
use crate::storage::StorageRequest;
use crate::task::{MetaTask, Range};
use crate::throttler::Throttler;
use crate::worker::Metadata;
use crate::{Result, TaskId};
use arc_swap::ArcSwap;
use rand::Rng;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

pub mod commands;
pub mod engine;
pub mod events;
pub mod lifecycle;

pub use engine::Engine;

/// Internal commands for the Orchestrator.
#[derive(Debug, Clone)]
pub enum Command {
    AddTask {
        id: TaskId,
        name: String,
        sources: Vec<(String, crate::task::TaskType)>,
    },
    Pause(TaskId),
    Resume(TaskId),
    Remove(TaskId),
    ListActive(mpsc::Sender<Vec<MetaTask>>),
    GetConfig(mpsc::Sender<Arc<crate::Config>>),
    ReloadConfig(Arc<crate::Config>),
    KillSwitch,
    Shutdown,
}

#[derive(Debug)]
pub enum SubTaskEvent {
    Matured(TaskId, TaskId, Metadata),
    MetadataReceived(TaskId, TaskId, crate::torrent::Torrent),
    RangeFinished(TaskId, TaskId, Range),
    Failed(TaskId, TaskId, String),
    Downloaded(TaskId, u64),
    PeerBitfield(TaskId, PeerId, Bitfield),
    PeerHave(TaskId, PeerId, u32),
    BtTaskRegistered(TaskId, TaskId, [u8; 20], Arc<BtTask>),
    KillSwitch,
}

/// Telemetry events published to the Event Bus.
#[derive(Debug, Clone, serde::Serialize)]
pub enum Event {
    TaskAdded(TaskId),
    MetadataResolved {
        id: TaskId,
        final_uri: String,
        total_length: u64,
        name: Option<String>,
    },
    TaskProgress {
        id: TaskId,
        completed_bytes: u64,
        total_bytes: u64,
    },
    TaskCompleted(TaskId),
    TaskError {
        id: TaskId,
        message: String,
    },
}

pub struct Orchestrator {
    pub(crate) tasks: HashMap<TaskId, MetaTask>,
    pub(crate) bt_registry: HashMap<[u8; 20], Arc<BtTask>>,
    pub(crate) bt_tasks: HashMap<TaskId, Arc<BtTask>>, // Key: sub_id
    pub(crate) cancellation_tokens: HashMap<TaskId, CancellationToken>,
    pub(crate) command_rx: mpsc::Receiver<Command>,
    pub(crate) event_tx: broadcast::Sender<Event>,
    pub(crate) storage_tx: mpsc::Sender<StorageRequest>,
    pub(crate) storage_completion_rx: mpsc::Receiver<TaskId>,
    pub(crate) subtask_tx: mpsc::Sender<SubTaskEvent>,
    pub(crate) subtask_rx: mpsc::Receiver<SubTaskEvent>,
    pub(crate) dht_tx: mpsc::Sender<DhtCommand>,
    pub(crate) _nat_tx: mpsc::Sender<NatCommand>,
    pub(crate) peer_id: [u8; 20],
    pub(crate) throttler: Arc<Throttler>,
    pub(crate) config: Arc<ArcSwap<crate::Config>>,
}

impl Orchestrator {
    pub(crate) fn resolve_local_addr(&self) -> Option<std::net::IpAddr> {
        let config = self.config.load();
        if let Some(addr) = config.network.local_addr {
            return Some(addr);
        }

        if let Some(ref iface) = config.network.interface {
            use local_ip_address::list_afinet_netifas;
            if let Ok(ifas) = list_afinet_netifas() {
                for (name, ip) in ifas {
                    if name == *iface {
                        return Some(ip);
                    }
                }
            }
        }

        None
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command_rx: mpsc::Receiver<Command>,
        storage_tx: mpsc::Sender<StorageRequest>,
        storage_completion_rx: mpsc::Receiver<TaskId>,
        dht_tx: mpsc::Sender<DhtCommand>,
        nat_tx: mpsc::Sender<NatCommand>,
        config: Arc<ArcSwap<crate::Config>>,
    ) -> (Self, broadcast::Receiver<Event>) {
        let (event_tx, event_rx) = broadcast::channel(1024);
        let (subtask_tx, subtask_rx) = mpsc::channel(4096);

        let mut peer_id = [0u8; 20];
        peer_id[..8].copy_from_slice(b"-AR0001-");
        rand::thread_rng().fill(&mut peer_id[8..]);

        let throttler = Arc::new(Throttler::new(0));

        (
            Self {
                tasks: HashMap::new(),
                bt_registry: HashMap::new(),
                bt_tasks: HashMap::new(),
                cancellation_tokens: HashMap::new(),
                command_rx,
                event_tx,
                storage_tx,
                storage_completion_rx,
                subtask_tx,
                subtask_rx,
                dht_tx,
                _nat_tx: nat_tx,
                peer_id,
                throttler,
                config,
            },
            event_rx,
        )
    }

    pub async fn run(mut self) -> Result<()> {
        tracing::info!("Orchestrator started");

        let local_addr = self.resolve_local_addr();
        let config_initial = self.config.load();
        let bind_addr = std::net::SocketAddr::new(
            local_addr.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED)),
            config_initial.network.listen_port,
        );

        let listener = tokio::net::TcpListener::bind(bind_addr)
            .await
            .map_err(|e| {
                crate::Error::Config(format!(
                    "Failed to bind Peer Listener on {}: {}",
                    bind_addr, e
                ))
            })?;
        tracing::info!("Peer Listener listening on {}", bind_addr);

        let mut save_interval = tokio::time::interval(std::time::Duration::from_secs(
            config_initial.storage.save_session_interval_secs,
        ));

        // VPN Kill-switch Monitor
        let config_monitor = self.config.clone();
        let subtask_tx_monitor = self.subtask_tx.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            loop {
                interval.tick().await;
                let config = config_monitor.load();
                if let Some(ref iface) = config.network.interface {
                    let iface_clone = iface.clone();
                    let is_up = tokio::task::spawn_blocking(move || {
                        use local_ip_address::list_afinet_netifas;
                        list_afinet_netifas()
                            .ok()
                            .map(|ifas| ifas.into_iter().any(|(name, _)| name == iface_clone))
                    })
                    .await
                    .unwrap_or(None)
                    .unwrap_or(false);

                    if !is_up {
                        tracing::warn!(
                            "VPN Kill-switch triggered! Interface {} is down. Stopping all tasks.",
                            iface
                        );
                        let _ = subtask_tx_monitor.send(SubTaskEvent::KillSwitch).await;
                    }
                }
            }
        });

        loop {
            tokio::select! {
                _ = save_interval.tick() => {
                    let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                    for id in ids {
                        let _ = self.save_task(id).await;
                    }
                }
                Ok((stream, addr)) = listener.accept() => {
                    let bt_registry = self.bt_registry.clone();
                    let storage_tx = self.storage_tx.clone();
                    let subtask_tx = self.subtask_tx.clone();
                    let my_peer_id = self.peer_id;
                    let cancellation_tokens = self.cancellation_tokens.clone();
                    let local_addr = self.resolve_local_addr();
                    let config = self.config.load().clone();

                    tokio::spawn(async move {
                        if let Err(e) = lifecycle::handle_incoming_peer(stream, addr, bt_registry, storage_tx, subtask_tx, my_peer_id, cancellation_tokens, local_addr, config).await {
                            tracing::debug!(?addr, error = %e, "Failed to handle incoming peer");
                        }
                    });
                }
                Some(event) = self.subtask_rx.recv() => {
                    if let Err(e) = self.handle_subtask_event(event).await {
                        tracing::error!("Event handle error: {}", e);
                    }
                }
                Some(cmd) = self.command_rx.recv() => {
                    if let Err(e) = self.handle_command(cmd).await {
                        tracing::error!("Command handle error: {}", e);
                    }
                }
                Some(id) = self.storage_completion_rx.recv() => {
                    if let Err(e) = self.handle_storage_completion(id).await {
                        tracing::error!("Storage completion error: {}", e);
                    }
                }
            }
        }
    }
}
