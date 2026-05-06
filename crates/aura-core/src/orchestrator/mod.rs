use crate::bitfield::Bitfield;
use crate::bt_task::BtTask;
use crate::bt_worker::PeerId;
use crate::buffer_pool::BufferPool;
use crate::dht::DhtCommand;
use crate::nat::NatCommand;
use crate::storage::StorageRequest;
use crate::task::{MetaTask, Range};
use crate::throttler::Throttler;
use crate::worker::Metadata;
use crate::{Result, TaskId};
use arc_swap::ArcSwap;
use rand::RngExt;
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

#[derive(Debug, Clone)]
pub enum WorkerCommand {
    CancelPiece(usize),
    RequestPiece(usize),
}

#[derive(Debug)]
pub enum SubTaskEvent {
    Matured(TaskId, TaskId, Metadata),
    MetadataReceived(TaskId, TaskId, crate::torrent::Torrent),
    RangeFinished(TaskId, TaskId, Range),
    Failed(TaskId, TaskId, String),
    Downloaded(TaskId, u64),
    Uploaded(TaskId, u64),
    PeerBitfield(TaskId, PeerId, Bitfield),
    PeerHave(TaskId, PeerId, u32),
    PieceVerified(TaskId, TaskId, usize),
    BtTaskRegistered(
        TaskId,
        TaskId,
        [u8; 20],
        Arc<BtTask>,
        tokio::sync::broadcast::Sender<WorkerCommand>,
    ),
    LpdPeerDiscovered([u8; 20], crate::tracker::Peer),
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
        uploaded_bytes: u64,
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
    pub(crate) worker_command_txs: HashMap<TaskId, tokio::sync::broadcast::Sender<WorkerCommand>>, // Key: sub_id
    pub(crate) cancellation_tokens: HashMap<TaskId, CancellationToken>,
    pub(crate) command_rx: mpsc::Receiver<Command>,
    pub(crate) event_tx: broadcast::Sender<Event>,
    pub(crate) storage_tx: mpsc::Sender<StorageRequest>,
    pub(crate) storage_completion_rx: mpsc::Receiver<TaskId>,
    pub(crate) subtask_tx: mpsc::Sender<SubTaskEvent>,
    pub(crate) subtask_rx: mpsc::Receiver<SubTaskEvent>,
    pub(crate) dht_tx: mpsc::Sender<DhtCommand>,
    pub(crate) lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
    pub(crate) _nat_tx: mpsc::Sender<NatCommand>,
    pub(crate) peer_id: [u8; 20],
    pub(crate) throttler: Arc<Throttler>,
    pub(crate) config: Arc<ArcSwap<crate::Config>>,
    pub(crate) power_manager: crate::power::PowerManager,
    pub(crate) pool: BufferPool,
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

    pub(crate) fn update_power_management(&mut self) {
        use crate::task::DownloadPhase;
        let is_active = self
            .tasks
            .values()
            .any(|t| t.phase == DownloadPhase::Downloading);
        self.power_manager.set_active(is_active);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command_rx: mpsc::Receiver<Command>,
        storage_tx: mpsc::Sender<StorageRequest>,
        storage_completion_rx: mpsc::Receiver<TaskId>,
        dht_tx: mpsc::Sender<DhtCommand>,
        lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
        nat_tx: mpsc::Sender<NatCommand>,
        config: Arc<ArcSwap<crate::Config>>,
        pool: BufferPool,
    ) -> (Self, broadcast::Sender<Event>) {
        let (event_tx, _event_rx) = broadcast::channel(1024);
        let (subtask_tx, subtask_rx) = mpsc::channel(4096);

        let mut peer_id = [0u8; 20];
        peer_id[..8].copy_from_slice(b"-AR0001-");
        rand::rng().fill(&mut peer_id[8..]);

        let throttler = Arc::new(Throttler::new(0));

        (
            Self {
                tasks: HashMap::new(),
                bt_registry: HashMap::new(),
                bt_tasks: HashMap::new(),
                worker_command_txs: HashMap::new(),
                cancellation_tokens: HashMap::new(),
                command_rx,
                event_tx: event_tx.clone(),
                storage_tx,
                storage_completion_rx,
                subtask_tx,
                subtask_rx,
                dht_tx,
                lpd_tx,
                _nat_tx: nat_tx,
                peer_id,
                throttler,
                config,
                power_manager: crate::power::PowerManager::new(),
                pool,
            },
            event_tx,
        )
    }

    pub(crate) async fn check_seed_limits(&mut self) {
        let config = self.config.load();
        let target_ratio = config.bittorrent.seed_ratio;
        let target_time = config.bittorrent.seed_time_mins as i64;

        let mut to_pause = Vec::new();
        for (id, task) in &self.tasks {
            if task.phase == crate::task::DownloadPhase::Complete {
                // Check Ratio
                if target_ratio > 0.0 {
                    let current_ratio = if task.completed_length > 0 {
                        task.uploaded_length as f64 / task.completed_length as f64
                    } else {
                        0.0
                    };
                    if current_ratio >= target_ratio as f64 {
                        tracing::info!(%id, current_ratio, target_ratio, "Seed ratio reached, pausing task");
                        to_pause.push(*id);
                        continue;
                    }
                }

                // Check Time
                if target_time > 0 {
                    if let Some(start_time) = task.seeding_start_time {
                        let elapsed = chrono::Utc::now() - start_time;
                        if elapsed.num_minutes() >= target_time {
                            tracing::info!(%id, elapsed_mins = elapsed.num_minutes(), target_time, "Seed time limit reached, pausing task");
                            to_pause.push(*id);
                        }
                    }
                }
            }
        }

        for id in to_pause {
            let _ = self.handle_pause(id).await;
        }
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
                    self.check_seed_limits().await;
                    let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                    for id in ids {
                        let _ = self.save_task(id).await;
                    }
                }
                Ok((stream, addr)) = listener.accept() => {
                    let bt_registry = self.bt_registry.clone();
                    let worker_command_txs = self.worker_command_txs.clone();
                    let storage_tx = self.storage_tx.clone();
                    let subtask_tx = self.subtask_tx.clone();
                    let my_peer_id = self.peer_id;
                    let cancellation_tokens = self.cancellation_tokens.clone();
                    let local_addr = self.resolve_local_addr();
                    let config = self.config.load().clone();
                    let pool = self.pool.clone();

                    tokio::spawn(async move {
                        if let Err(e) = lifecycle::handle_incoming_peer(stream, addr, bt_registry, worker_command_txs, storage_tx, subtask_tx, my_peer_id, cancellation_tokens, local_addr, config, pool).await {
                            tracing::debug!(?addr, error = %e, "Failed to handle incoming peer");
                        }
                    });
                }
                Some(event) = self.subtask_rx.recv() => {
                    if let Err(e) = self.handle_subtask_event(event).await {
                        tracing::error!("Event handle error: {}", e);
                    }
                    self.update_power_management();
                }
                Some(cmd) = self.command_rx.recv() => {
                    if let Err(e) = self.handle_command(cmd).await {
                        tracing::error!("Command handle error: {}", e);
                    }
                    self.update_power_management();
                }
                Some(id) = self.storage_completion_rx.recv() => {
                    if let Err(e) = self.handle_storage_completion(id).await {
                        tracing::error!("Storage completion error: {}", e);
                    }
                    self.update_power_management();
                }
            }
        }
    }
}
