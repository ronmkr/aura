use std::collections::HashMap;
use std::sync::Arc;
use std::net::SocketAddr;
use tokio::sync::{mpsc, broadcast};
use tokio_util::sync::CancellationToken;
use tracing::{info, debug, warn};
use rand::Rng;
use crate::{Result, TaskId, Error};
use crate::task::{MetaTask, TaskType, Range, TaskState};
use crate::storage::{StorageEngine, StorageRequest};
use crate::worker::{HttpWorker, ProtocolWorker, Segment, Metadata};
use crate::bt_task::BtTask;
use crate::throttler::Throttler;
use crate::bt_worker::{BtWorker, PeerId};
use crate::bitfield::Bitfield;
use crate::dht::DhtCommand;
use crate::nat::NatCommand;

use serde_json;

/// Internal commands for the Orchestrator.
#[derive(Debug, Clone)]
pub enum Command {
    AddTask {
        id: TaskId,
        name: String,
        sources: Vec<(String, TaskType)>,
    },
    Pause(TaskId),
    Resume(TaskId),
    Remove(TaskId),
    ListActive(mpsc::Sender<Vec<MetaTask>>),
    KillSwitch,
    Shutdown,
}

#[derive(Debug)]
pub enum SubTaskEvent {
    Matured(TaskId, TaskId, Metadata),
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
    MetadataResolved { id: TaskId, final_uri: String, total_length: u64, name: Option<String> },
    TaskProgress { id: TaskId, completed_bytes: u64, total_bytes: u64 },
    TaskCompleted(TaskId),
    TaskError { id: TaskId, message: String },
}

pub struct Orchestrator {
    tasks: HashMap<TaskId, MetaTask>,
    bt_registry: HashMap<[u8; 20], Arc<BtTask>>,
    bt_tasks: HashMap<TaskId, Arc<BtTask>>, // Key: sub_id
    cancellation_tokens: HashMap<TaskId, CancellationToken>,
    command_rx: mpsc::Receiver<Command>,
    event_tx: broadcast::Sender<Event>,
    storage_tx: mpsc::Sender<StorageRequest>,
    storage_completion_rx: mpsc::Receiver<TaskId>,
    subtask_tx: mpsc::UnboundedSender<SubTaskEvent>,
    subtask_rx: mpsc::UnboundedReceiver<SubTaskEvent>,
    dht_tx: mpsc::Sender<DhtCommand>,
    _nat_tx: mpsc::Sender<NatCommand>,
    peer_id: [u8; 20],
    throttler: Arc<Throttler>,
    config: Arc<crate::Config>,
}

#[allow(clippy::too_many_arguments)]
async fn handle_incoming_peer(
    mut stream: tokio::net::TcpStream,
    addr: std::net::SocketAddr,
    bt_registry: HashMap<[u8; 20], Arc<BtTask>>,
    storage_tx: mpsc::Sender<StorageRequest>,
    subtask_tx: mpsc::UnboundedSender<SubTaskEvent>,
    my_peer_id: [u8; 20],
    cancellation_tokens: HashMap<TaskId, CancellationToken>,
    local_addr: Option<std::net::IpAddr>,
) -> Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use crate::bt_worker::HANDSHAKE_LEN;
    use crate::bt_worker::Handshake;

    let mut buf = [0u8; HANDSHAKE_LEN];
    stream.read_exact(&mut buf).await?;
    let handshake = Handshake::deserialize(&buf)?;

    if let Some(task) = bt_registry.get(&handshake.info_hash) {
        if let Some(token) = cancellation_tokens.get(&task.id) {
            if token.is_cancelled() {
                return Ok(());
            }

            info!(?addr, "Accepted incoming peer for task {}", task.id);
            
            let my_handshake = Handshake::new(handshake.info_hash, my_peer_id);
            stream.write_all(&my_handshake.serialize()).await?;

            let mut worker = BtWorker::new(addr.to_string(), handshake.info_hash, handshake.peer_id, my_peer_id);
            worker.local_addr = local_addr;
            worker.run_loop_with_stream(stream, task.id, task.id, task.clone(), storage_tx, subtask_tx, token.clone()).await
        } else {
            Ok(())
        }
    } else {
        debug!(?addr, "Rejected incoming peer: unknown info_hash");
        Ok(())
    }
}

impl Orchestrator {
    fn resolve_local_addr(&self) -> Option<std::net::IpAddr> {
        if let Some(addr) = self.config.network.local_addr {
            return Some(addr);
        }

        if let Some(ref iface) = self.config.network.interface {
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

    pub fn new(
        command_rx: mpsc::Receiver<Command>,
        storage_tx: mpsc::Sender<StorageRequest>,
        storage_completion_rx: mpsc::Receiver<TaskId>,
        dht_tx: mpsc::Sender<DhtCommand>,
        nat_tx: mpsc::Sender<NatCommand>,
        config: Arc<crate::Config>,
    ) -> (Self, broadcast::Receiver<Event>) {
        let (event_tx, event_rx) = broadcast::channel(1024);
        let (subtask_tx, subtask_rx) = mpsc::unbounded_channel();
        
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
        info!("Orchestrator started");
        
        let local_addr = self.resolve_local_addr();
        let bind_addr = SocketAddr::new(local_addr.unwrap_or("0.0.0.0".parse().unwrap()), 6881);
        
        let listener = tokio::net::TcpListener::bind(bind_addr).await
            .map_err(|e| Error::Config(format!("Failed to bind Peer Listener on {}: {}", bind_addr, e)))?;
        info!("Peer Listener listening on {}", bind_addr);

        let mut save_interval = tokio::time::interval(std::time::Duration::from_secs(10));
        
        // VPN Kill-switch Monitor
        let config_monitor = self.config.clone();
        let subtask_tx_monitor = self.subtask_tx.clone();
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            loop {
                interval.tick().await;
                if let Some(ref iface) = config_monitor.network.interface {
                    use local_ip_address::list_afinet_netifas;
                    let is_up = list_afinet_netifas().ok().map(|ifas: Vec<(String, std::net::IpAddr)>| {
                        ifas.into_iter().any(|(name, _)| name == *iface)
                    }).unwrap_or(false);

                    if !is_up {
                        warn!("VPN Kill-switch triggered! Interface {} is down. Stopping all tasks.", iface);
                        let _ = subtask_tx_monitor.send(SubTaskEvent::KillSwitch);
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

                    tokio::spawn(async move {
                        if let Err(e) = handle_incoming_peer(stream, addr, bt_registry, storage_tx, subtask_tx, my_peer_id, cancellation_tokens, local_addr).await {
                            debug!(?addr, error = %e, "Failed to handle incoming peer");
                        }
                    });
                }
                Some(event) = self.subtask_rx.recv() => {
                    match event {
                        SubTaskEvent::Matured(meta_id, sub_id, metadata) => {
                            self.handle_subtask_matured(meta_id, sub_id, metadata).await?;
                        }
                        SubTaskEvent::RangeFinished(meta_id, sub_id, range) => {
                            self.handle_range_finished(meta_id, sub_id, range).await?;
                        }
                        SubTaskEvent::Failed(meta_id, sub_id, err) => {
                            info!(%meta_id, %sub_id, %err, "Subtask failed");
                        }
                        SubTaskEvent::Downloaded(meta_id, bytes) => {
                            self.throttler.consume_download(bytes).await;
                            if let Some(task) = self.tasks.get_mut(&meta_id) {
                                task.completed_length += bytes;
                                let _ = self.event_tx.send(Event::TaskProgress {
                                    id: meta_id,
                                    completed_bytes: task.completed_length,
                                    total_bytes: task.total_length,
                                });
                            }
                        }
                        SubTaskEvent::PeerBitfield(meta_id, peer_id, bf) => {
                            debug!(%meta_id, ?peer_id, count = bf.count_set(), "Peer bitfield received");
                        }
                        SubTaskEvent::PeerHave(meta_id, peer_id, idx) => {
                            debug!(%meta_id, ?peer_id, idx, "Peer reported piece availability");
                        }
                        SubTaskEvent::BtTaskRegistered(_meta_id, sub_id, info_hash, task) => {
                            self.bt_registry.insert(info_hash, task.clone());
                            self.bt_tasks.insert(sub_id, task);
                        }
                        SubTaskEvent::KillSwitch => {
                            let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                            for id in ids {
                                let _ = self.handle_pause(id).await;
                            }
                        }
                    }
                }
                Some(cmd) = self.command_rx.recv() => {
                    match cmd {
                        Command::AddTask { id, name, sources } => {
                            self.handle_add_task(id, name, sources).await?;
                        }
                        Command::Pause(id) => {
                            self.handle_pause(id).await?;
                        }
                        Command::Resume(id) => {
                            self.handle_resume(id).await?;
                        }
                        Command::Remove(id) => {
                            let _ = self.handle_pause(id).await;
                            self.tasks.remove(&id);
                        }
                        Command::ListActive(reply_tx) => {
                            let active: Vec<MetaTask> = self.tasks.values().cloned().collect();
                            let _ = reply_tx.send(active).await;
                        }
                        Command::KillSwitch => {
                            let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                            for id in ids {
                                let _ = self.handle_pause(id).await;
                            }
                        }
                        Command::Shutdown => {
                            info!("Orchestrator shutting down");
                            return Ok(());
                        }
                    }
                }
                Some(id) = self.storage_completion_rx.recv() => {
                    info!(%id, "Storage reported completion");
                    if let Some(task) = self.tasks.get(&id) {
                        let _ = self.event_tx.send(Event::TaskProgress { 
                            id, 
                            completed_bytes: task.total_length,
                            total_bytes: task.total_length 
                        });
                    }
                    let _ = self.event_tx.send(Event::TaskCompleted(id));
                }
            }
        }
    }

    async fn handle_pause(&mut self, id: TaskId) -> Result<()> {
        if let Some(task) = self.tasks.get_mut(&id) {
            if task.phase != crate::task::DownloadPhase::Paused && task.phase != crate::task::DownloadPhase::Complete {
                info!(%id, "Pausing task");
                task.phase = crate::task::DownloadPhase::Paused;
                
                if let Some(token) = self.cancellation_tokens.remove(&id) {
                    token.cancel();
                }

                // Recycle in-flight ranges
                let in_flight = std::mem::take(&mut task.in_flight_ranges);
                for (_sub_id, range) in in_flight {
                    task.pending_ranges.push(range);
                }

                let _ = self.event_tx.send(Event::TaskProgress {
                    id,
                    completed_bytes: task.completed_length,
                    total_bytes: task.total_length,
                });
            }
        }
        Ok(())
    }

    async fn handle_resume(&mut self, id: TaskId) -> Result<()> {
        if let Some(task) = self.tasks.get_mut(&id) {
            if task.phase == crate::task::DownloadPhase::Paused {
                info!(%id, "Resuming task");
                task.phase = crate::task::DownloadPhase::Downloading;
                
                let token = CancellationToken::new();
                self.cancellation_tokens.insert(id, token.clone());
                
                self.start_task_loops(id, token).await?;
            }
        }
        Ok(())
    }

    async fn save_task(&self, id: TaskId) -> Result<()> {
        if let Some(meta_task) = self.tasks.get(&id) {
            let mut bitfield = None;
            for sub in &meta_task.subtasks {
                if let Some(bt) = self.bt_tasks.get(&sub.id) {
                    let bf = bt.state.bitfield.lock().await;
                    bitfield = Some(bf.clone());
                    break;
                }
            }

            let state = meta_task.to_state(bitfield);
            let filename = format!("{}.aura", meta_task.name);
            let path = std::env::current_dir().unwrap_or_default().join(&filename);
            info!(%id, path = ?path, "Saving control file");
            
            let data = serde_json::to_vec_pretty(&state)
                .map_err(|e| Error::Storage(format!("Failed to serialize task state: {}", e)))?;
            
            tokio::fs::write(&path, data).await
                .map_err(|e| Error::Storage(format!("Failed to write control file {}: {}", filename, e)))?;
        }
        Ok(())
    }

    async fn handle_add_task(&mut self, id: TaskId, name: String, sources: Vec<(String, TaskType)>) -> Result<()> {
        info!(%id, %name, "Adding MetaTask with {} sources", sources.len());
        
        let path = format!("{}.aura", name);
        let (mut meta_task, loaded_bitfield) = if let Ok(data) = tokio::fs::read(&path).await {
            match serde_json::from_slice::<TaskState>(&data) {
                Ok(state) => {
                    info!(%id, "Resuming task from control file {}", path);
                    let bitfield = state.bitfield.clone();
                    let mut mt = MetaTask::from_state(state);
                    mt.id = id; // Update to the new ID
                    (mt, bitfield)
                }
                Err(e) => {
                    warn!(%id, "Failed to parse control file {}: {}. Starting fresh.", path, e);
                    (MetaTask::new(id, name, 0), None)
                }
            }
        } else {
            (MetaTask::new(id, name, 0), None)
        };

        if meta_task.subtasks.is_empty() {
            for (uri, ttype) in sources {
                meta_task.add_subtask(uri, ttype);
            }
        }
        
        let token = CancellationToken::new();
        self.cancellation_tokens.insert(id, token.clone());

        self.tasks.insert(id, meta_task);
        self.start_task_loops_with_bitfield(id, token, loaded_bitfield).await?;

        let _ = self.event_tx.send(Event::TaskAdded(id));
        Ok(())
    }

    async fn start_task_loops(&mut self, id: TaskId, token: CancellationToken) -> Result<()> {
        self.start_task_loops_with_bitfield(id, token, None).await
    }

    async fn start_task_loops_with_bitfield(&mut self, id: TaskId, token: CancellationToken, bitfield: Option<Bitfield>) -> Result<()> {
        let meta_task = self.tasks.get(&id).ok_or_else(|| Error::Config("Task not found".to_string()))?;
        let subtasks = meta_task.subtasks.clone();
        let my_peer_id = self.peer_id;
        let local_addr = self.resolve_local_addr();

        for sub_task in subtasks {
            let sub_id = sub_task.id;
            let uri = sub_task.uri.clone();
            let ttype = sub_task.task_type.clone();
            let subtask_tx = self.subtask_tx.clone();
            let storage_tx = self.storage_tx.clone();
            let dht_tx = self.dht_tx.clone();
            let token = token.clone();
            let loaded_bf = bitfield.clone();
            
            // Try to reuse existing BT state
            let existing_bt = self.bt_tasks.get(&sub_id).cloned();

            tokio::spawn(async move {
                match ttype {
                    TaskType::Http => {
                        let worker = HttpWorker::new(uri, local_addr);
                        match worker.resolve_metadata().await {
                            Ok(m) => {
                                let _ = subtask_tx.send(SubTaskEvent::Matured(id, sub_id, m));
                            }
                            Err(e) => {
                                let _ = subtask_tx.send(SubTaskEvent::Failed(id, sub_id, e.to_string()));
                            }
                        }
                    }
                    TaskType::Ftp => {
                        let worker = crate::worker::FtpWorker::new(uri, local_addr);
                        match worker.resolve_metadata().await {
                            Ok(m) => {
                                let _ = subtask_tx.send(SubTaskEvent::Matured(id, sub_id, m));
                            }
                            Err(e) => {
                                let _ = subtask_tx.send(SubTaskEvent::Failed(id, sub_id, e.to_string()));
                            }
                        }
                    }
                    TaskType::BitTorrent => {
                        let bt_task = if let Some(bt) = existing_bt {
                            bt
                        } else {
                            match BtTask::from_file(id, &uri, dht_tx).await {
                                Ok(t) => {
                                    if let Some(bf) = loaded_bf {
                                        let mut my_bf = t.state.bitfield.lock().await;
                                        *my_bf = bf;
                                        let mut picker = t.state.picker.lock().await;
                                        for i in 0..my_bf.len() {
                                            if my_bf.get(i) {
                                                picker.mark_completed(i);
                                            }
                                        }
                                    }
                                    Arc::new(t)
                                }
                                Err(e) => {
                                    let _ = subtask_tx.send(SubTaskEvent::Failed(id, sub_id, e.to_string()));
                                    return;
                                }
                            }
                        };
                        
                        let info_hash = bt_task.state.torrent.info_hash().unwrap_or([0; 20]);
                        let _ = subtask_tx.send(SubTaskEvent::BtTaskRegistered(id, sub_id, info_hash, bt_task.clone()));
                        
                        let total_length = bt_task.state.torrent.total_length();
                        let metadata = Metadata {
                            final_uri: uri.clone(),
                            total_length: Some(total_length),
                            name: Some(bt_task.state.torrent.info.name.clone()),
                        };
                        let _ = subtask_tx.send(SubTaskEvent::Matured(id, sub_id, metadata));

                        // Start tracker loop
                        let tracker_task = bt_task.clone();
                        let t1 = token.clone();
                        tokio::spawn(async move {
                            let _ = tracker_task.run_tracker_loop(my_peer_id, 6881, t1, local_addr).await;
                        });

                        // Start DHT loop
                        let dht_task = bt_task.clone();
                        let t2 = token.clone();
                        tokio::spawn(async move {
                            let _ = dht_task.run_dht_loop(t2).await;
                        });

                        // Start peer connection loop
                        let peer_task = bt_task.clone();
                        let storage_tx_loop = storage_tx.clone();
                        let subtask_tx_loop = subtask_tx.clone();
                        let info_hash = bt_task.state.torrent.info_hash().unwrap_or([0; 20]);
                        let t3 = token.clone();

                        use std::sync::atomic::{AtomicUsize, Ordering};
                        let active_workers = Arc::new(AtomicUsize::new(0));

                        tokio::spawn(async move {
                            loop {
                                if t3.is_cancelled() { break; }
                                if active_workers.load(Ordering::Relaxed) < 50 {
                                    if let Some((maybe_piece_idx, peer)) = peer_task.pick_work().await {
                                        let addr = format!("{}:{}", peer.ip, peer.port);
                                        let peer_id = peer.id.and_then(|v| {
                                            if let serde_bencode::value::Value::Bytes(b) = v {
                                                let mut pid = [0u8; 20];
                                                if b.len() == 20 {
                                                    pid.copy_from_slice(&b);
                                                    Some(pid)
                                                } else { None }
                                            } else { None }
                                        }).unwrap_or([0; 20]);

                                        info!(%id, %addr, ?maybe_piece_idx, "Initiating peer connection");
                                        peer_task.update_peer_state(&addr, crate::peer_registry::ConnectionState::Connecting).await;

                                        let mut worker = BtWorker::new(addr.clone(), info_hash, peer_id, my_peer_id);
                                        worker.local_addr = local_addr;
                                        let s_tx = storage_tx_loop.clone();
                                        let sub_tx = subtask_tx_loop.clone();
                                        let peer_task_inner = peer_task.clone();
                                        let active_counter = active_workers.clone();
                                        let t4 = t3.clone();

                                        active_counter.fetch_add(1, Ordering::Relaxed);
                                        tokio::spawn(async move {
                                            if let Err(e) = worker.run_loop(id, sub_id, peer_task_inner, s_tx, sub_tx, t4).await {
                                                debug!(%id, %addr, error = %e, "Peer loop stopped");
                                            }
                                            active_counter.fetch_sub(1, Ordering::Relaxed);
                                        });
                                    }
                                }
                                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                            }
                        });
                    }
                }
            });
        }
        Ok(())
    }

    async fn handle_subtask_matured(&mut self, meta_id: TaskId, sub_id: TaskId, metadata: Metadata) -> Result<()> {
        let mut initialized = false;
        let mut matured_uri = String::new();
        let mut total_len = 0;
        let mut matured_name = None;

        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
            let total_length = metadata.total_length.unwrap_or(0);
            
            if let Some(ref name) = metadata.name {
                meta_task.name = name.clone();
                matured_name = Some(name.clone());
            }

            if meta_task.total_length == 0 && total_length > 0 {
                meta_task.total_length = total_length;
                meta_task.initialize_ranges(64);
                initialized = true;
                matured_uri = metadata.final_uri.clone();
                total_len = total_length;
            }

            if let Some(sub_task) = meta_task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                sub_task.total_length = total_length;
            }
        }

        if initialized {
            let _ = self.event_tx.send(Event::MetadataResolved {
                id: meta_id,
                final_uri: matured_uri,
                total_length: total_len,
                name: matured_name,
            });
            self.dispatch_next_ranges(meta_id, sub_id).await?;
            // Save immediately after maturation
            let _ = self.save_task(meta_id).await;
        }

        Ok(())
    }

    async fn handle_range_finished(&mut self, meta_id: TaskId, sub_id: TaskId, range: Range) -> Result<()> {
        let mut progress = 0;
        let mut total = 0;
        let mut finished = false;

        if let Some(meta_task) = self.tasks.get_mut(&meta_id) {
            meta_task.completed_length += range.length();
            meta_task.in_flight_ranges.retain(|(sid, r)| *sid != sub_id || *r != range);

            if let Some(sub) = meta_task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                sub.assigned_ranges.retain(|r| *r != range);
                sub.completed_length += range.length();
            }

            progress = meta_task.completed_length;
            total = meta_task.total_length;
            finished = meta_task.completed_length >= meta_task.total_length;
        }

        let _ = self.event_tx.send(Event::TaskProgress {
            id: meta_id,
            completed_bytes: progress,
            total_bytes: total
        });

        self.dispatch_next_ranges(meta_id, sub_id).await?;
        
        let _ = self.save_task(meta_id).await;

        if finished {
            info!(%meta_id, "Logical download finished, triggering atomic completion");
            let _ = self.storage_tx.send(StorageRequest::Complete(meta_id)).await;
            
            // Remove control file on success
            if let Some(task) = self.tasks.get(&meta_id) {
                let path = format!("{}.aura", task.name);
                let _ = tokio::fs::remove_file(path).await;
            }
        }

        Ok(())
    }

    async fn dispatch_next_ranges(&mut self, meta_id: TaskId, sub_id: TaskId) -> Result<()> {
        let token = match self.cancellation_tokens.get(&meta_id) {
            Some(t) => t.clone(),
            None => return Ok(()),
        };

        if token.is_cancelled() {
            return Ok(());
        }

        let local_addr = self.resolve_local_addr();
        let concurrency_per_subtask = 4;

        loop {
            if token.is_cancelled() {
                break;
            }

            let meta_task = self.tasks.get_mut(&meta_id).ok_or_else(|| Error::Config("Task not found".to_string()))?;
            
            let (uri, ttype, current_concurrency) = {
                let sub_task = meta_task.subtasks.iter().find(|s| s.id == sub_id).ok_or_else(|| Error::Config("Subtask not found".to_string()))?;
                (sub_task.uri.clone(), sub_task.task_type.clone(), sub_task.assigned_ranges.len())
            };
            
            if current_concurrency >= concurrency_per_subtask {
                break;
            }

            if let Some(range) = meta_task.pick_range_for_subtask(sub_id) {
                let storage_tx = self.storage_tx.clone();
                let subtask_tx = self.subtask_tx.clone();
                let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
                let token = token.clone();
                
                let subtask_tx_progress = subtask_tx.clone();
                tokio::spawn(async move {
                    while let Some(bytes) = progress_rx.recv().await {
                        let _ = subtask_tx_progress.send(SubTaskEvent::Downloaded(meta_id, bytes));
                    }
                });

                tokio::spawn(async move {
                    match ttype {
                        TaskType::Http => {
                            let worker = HttpWorker::new(uri, local_addr);
                            let segment = Segment { offset: range.start, length: range.length() };
                            
                            tokio::select! {
                                _ = token.cancelled() => {
                                    // Range will be recycled by Orchestrator on pause
                                }
                                res = worker.fetch_segment(meta_id, segment, Some(progress_tx)) => {
                                    match res {
                                        Ok(piece) => {
                                            let _ = storage_tx.send(StorageRequest::Write {
                                                task_id: meta_id,
                                                segment: piece.segment,
                                                data: piece.data,
                                            }).await;
                                            let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, range));
                                        }
                                        Err(e) => {
                                            debug!(%meta_id, %sub_id, error = %e, "Range fetch failed");
                                            let _ = subtask_tx.send(SubTaskEvent::Failed(meta_id, sub_id, e.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                        TaskType::Ftp => {
                            let worker = crate::worker::FtpWorker::new(uri, local_addr);
                            let segment = Segment { offset: range.start, length: range.length() };
                            
                            tokio::select! {
                                _ = token.cancelled() => {
                                }
                                res = worker.fetch_segment(meta_id, segment, Some(progress_tx)) => {
                                    match res {
                                        Ok(piece) => {
                                            let _ = storage_tx.send(StorageRequest::Write {
                                                task_id: meta_id,
                                                segment: piece.segment,
                                                data: piece.data,
                                            }).await;
                                            let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, range));
                                        }
                                        Err(e) => {
                                            debug!(%meta_id, %sub_id, error = %e, "Range fetch failed");
                                            let _ = subtask_tx.send(SubTaskEvent::Failed(meta_id, sub_id, e.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                        TaskType::BitTorrent => {
                        }
                    }
                });
            } else {
                break;
            }
        }
        Ok(())
    }
}

/// The high-level Engine API for Aura.
pub struct Engine {
    command_tx: mpsc::Sender<Command>,
    event_rx: broadcast::Receiver<Event>,
}

impl Engine {
    pub async fn new(config: crate::Config) -> Result<(Self, Orchestrator, StorageEngine)> {
        let config = Arc::new(config);
        let (command_tx, command_rx) = mpsc::channel(100);
        let (storage_tx, storage_rx) = mpsc::channel(100);
        let (completion_tx, completion_rx) = mpsc::channel(100);
        let (dht_tx, dht_rx) = mpsc::channel(100);
        let (nat_tx, nat_rx) = mpsc::channel(100);
        
        let local_addr = {
            if let Some(addr) = config.network.local_addr {
                Some(addr)
            } else if let Some(ref iface) = config.network.interface {
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
        
        let dht_actor = DhtActor::new("0.0.0.0:6881", dht_id, dht_rx, local_addr).await?;
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
            port: 6881,
            description: "Aura BitTorrent".to_string(),
        }).await;

        let storage = StorageEngine::new(storage_rx, completion_tx);
        let (orchestrator, event_rx) = Orchestrator::new(command_rx, storage_tx, completion_rx, dht_tx, nat_tx, config);
        
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
                let state: TaskState = serde_json::from_slice(&data)
                    .map_err(|e| Error::Storage(e.to_string()))?;
                
                info!("Found persisted task: {}", state.name);
                // We'll need a new command to load a task state
                // self.command_tx.send(Command::LoadTask(state)).await?;
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
    }
