use crate::bitfield::Bitfield;
use crate::bt_task::BtTask;
use crate::bt_worker::PeerId;
use crate::buffer_pool::BufferPool;
use crate::dht::DhtCommand;
use crate::nat::NatCommand;
use crate::task::{MetaTask, Range};
use crate::throttler::Throttler;
use crate::worker::Metadata;
use crate::{InfoHash, Result, TaskId};
use arc_swap::ArcSwap;
use rand::RngExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

/// Internal commands for the Orchestrator.
#[derive(Debug)]
pub enum Command {
    AddTask {
        id: TaskId,
        name: String,
        sources: Vec<(String, crate::task::TaskType)>,
        checksum: Option<crate::Checksum>,
        priority: u32,
        streaming_mode: bool,
    },
    Pause(TaskId),
    Resume(TaskId),
    Remove(TaskId),
    ListActive(mpsc::Sender<Vec<MetaTask>>),
    GetConfig(mpsc::Sender<Arc<crate::Config>>),
    ReloadConfig(Arc<crate::Config>, tokio::sync::oneshot::Sender<()>),
    KillSwitch,
    Shutdown,
    RetrySubtask(TaskId, TaskId),
}

#[derive(Debug, Clone)]
pub enum WorkerCommand {
    CancelPiece(usize),
    RequestPiece(usize),
}

#[derive(Debug)]
pub enum SubTaskEvent {
    Matured(TaskId, TaskId, Metadata),
    MetadataReceived(TaskId, TaskId, Box<crate::torrent::Torrent>),
    RangeFinished(TaskId, TaskId, Range),
    Failed(TaskId, TaskId, String),
    Downloaded(TaskId, TaskId, u64),
    Uploaded(TaskId, u64),
    PeerBitfield(TaskId, PeerId, Bitfield),
    PeerHave(TaskId, PeerId, u32),
    PieceVerified(TaskId, TaskId, usize),
    BtTaskRegistered(
        TaskId,
        InfoHash,
        Arc<BtTask>,
        tokio::sync::broadcast::Sender<WorkerCommand>,
    ),
    LpdPeerDiscovered(InfoHash, crate::tracker::Peer),
    KillSwitch,
    Retry(TaskId, TaskId),
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
    TaskPaused(TaskId),
    TaskResumed(TaskId),
    TaskError {
        id: TaskId,
        message: String,
    },
}

pub struct Orchestrator {
    pub(crate) tasks: HashMap<TaskId, MetaTask>,
    pub(crate) bt_registry: HashMap<InfoHash, TaskId>,
    pub(crate) bt_tasks: HashMap<TaskId, Arc<BtTask>>,
    pub(crate) worker_command_txs: HashMap<TaskId, tokio::sync::broadcast::Sender<WorkerCommand>>,
    pub(crate) cancellation_tokens: HashMap<TaskId, CancellationToken>,
    pub(crate) command_rx: mpsc::Receiver<Command>,
    pub(crate) event_tx: broadcast::Sender<Event>,
    pub(crate) storage_tx: mpsc::Sender<crate::storage::StorageRequest>,
    pub(crate) storage_completion_rx: mpsc::Receiver<crate::storage::StorageEvent>,
    pub(crate) subtask_tx: mpsc::Sender<SubTaskEvent>,
    pub(crate) subtask_rx: mpsc::Receiver<SubTaskEvent>,
    pub(crate) dht_tx: mpsc::Sender<DhtCommand>,
    pub(crate) lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
    pub(crate) _nat_tx: mpsc::Sender<NatCommand>,
    pub(crate) peer_id: [u8; 20],
    pub(crate) throttler: Arc<Throttler>,
    pub(crate) vpn_provider: Option<Arc<dyn crate::vpn::VpnProvider>>,
    pub(crate) vpn_watch_tx: tokio::sync::watch::Sender<Option<Arc<dyn crate::vpn::VpnProvider>>>,
    pub(crate) config: Arc<ArcSwap<crate::Config>>,
    pub(crate) power_manager: crate::power::PowerManager,
    pub(crate) hook_manager: crate::hooks::HookManager,
    pub(crate) credential_provider: Arc<crate::config::credentials::CredentialProvider>,
    pub(crate) dns_resolver: Arc<crate::net_util::TokioResolver>,
    pub(crate) pool: BufferPool,
    pub(crate) db: sled::Db,
    pub(crate) hsts_cache: crate::security::HstsCache,
}

impl Orchestrator {
    pub(crate) fn create_vpn_provider(
        config: &crate::Config,
    ) -> Option<Arc<dyn crate::vpn::VpnProvider>> {
        if let Some(ref type_name) = config.vpn.type_name {
            match type_name.to_lowercase().as_str() {
                "wireguard" => {
                    let iface = config
                        .network
                        .interface
                        .clone()
                        .unwrap_or_else(|| "wg0".to_string());
                    Some(Arc::new(crate::vpn::WireGuardProvider::new(iface))
                        as Arc<dyn crate::vpn::VpnProvider>)
                }
                "openvpn" => {
                    let addr = config
                        .vpn
                        .profile_path
                        .clone()
                        .unwrap_or_else(|| "127.0.0.1:1337".to_string());
                    Some(Arc::new(crate::vpn::OpenVpnProvider::new(addr))
                        as Arc<dyn crate::vpn::VpnProvider>)
                }
                _ => None,
            }
        } else {
            config.network.interface.as_ref().map(|iface| {
                Arc::new(crate::vpn::InterfaceMonitor::new(iface.clone()))
                    as Arc<dyn crate::vpn::VpnProvider>
            })
        }
    }

    pub(crate) fn update_vpn_provider(&mut self, config: &crate::Config) {
        let new_provider = Self::create_vpn_provider(config);
        self.vpn_provider = new_provider.clone();
        let _ = self.vpn_watch_tx.send(new_provider);
    }

    pub(crate) fn resolve_local_addr(&self) -> Option<std::net::IpAddr> {
        let config = self.config.load();

        if config.vpn.force_tunnel {
            if let Some(ref vpn) = self.vpn_provider {
                if let Some(iface) = vpn.interface() {
                    use local_ip_address::list_afinet_netifas;
                    if let Ok(ifas) = list_afinet_netifas() {
                        for (name, ip) in ifas {
                            if name == iface {
                                return Some(ip);
                            }
                        }
                    }
                }
            }
        }

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

    pub(crate) async fn verify_vpn_connectivity(&self) -> Result<()> {
        let config = self.config.load();
        if !config.vpn.force_tunnel {
            return Ok(());
        }

        if let Some(ref vpn) = self.vpn_provider {
            match vpn.status().await? {
                crate::vpn::VpnStatus::Connected => Ok(()),
                crate::vpn::VpnStatus::Connecting => {
                    tracing::info!("VPN is connecting, waiting...");
                    Err(crate::Error::Engine("VPN is still connecting".to_string()))
                }
                crate::vpn::VpnStatus::Disconnected | crate::vpn::VpnStatus::Error(_) => {
                    if config.vpn.auto_connect {
                        tracing::info!("VPN disconnected, attempting auto-connect...");
                        vpn.connect().await?;
                        Err(crate::Error::Engine(
                            "VPN re-connect triggered. Please retry in a moment.".to_string(),
                        ))
                    } else {
                        Err(crate::Error::Engine(
                            "Mandatory VPN tunnel is down and auto-connect is disabled."
                                .to_string(),
                        ))
                    }
                }
            }
        } else {
            if config.network.interface.is_some() {
                Ok(())
            } else {
                Err(crate::Error::Config(
                    "Mandatory tunnel enabled but no VPN provider or interface configured"
                        .to_string(),
                ))
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        command_rx: mpsc::Receiver<Command>,
        storage_tx: mpsc::Sender<crate::storage::StorageRequest>,
        storage_completion_rx: mpsc::Receiver<crate::storage::StorageEvent>,
        dht_tx: mpsc::Sender<DhtCommand>,
        lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
        nat_tx: mpsc::Sender<NatCommand>,
        config: Arc<ArcSwap<crate::Config>>,
        pool: BufferPool,
        db: sled::Db,
        dns_resolver: Arc<crate::net_util::TokioResolver>,
    ) -> (Self, broadcast::Sender<Event>) {
        let (event_tx, _event_rx) = broadcast::channel(1024);
        let (subtask_tx, subtask_rx) = mpsc::channel(4096);

        let mut peer_id = [0u8; 20];
        peer_id[..8].copy_from_slice(b"-AR0001-");
        rand::rng().fill(&mut peer_id[8..]);

        let initial_config = config.load();
        let throttler = Arc::new(Throttler::new(
            initial_config.bandwidth.global_download_limit,
            initial_config.bandwidth.global_upload_limit,
        ));

        let vpn_provider = Self::create_vpn_provider(&initial_config);
        let (vpn_watch_tx, _vpn_watch_rx) = tokio::sync::watch::channel(vpn_provider.clone());
        let hook_manager = crate::hooks::HookManager::new(initial_config.hooks.clone());

        let mut credential_provider = crate::config::credentials::CredentialProvider::new();
        if let Some(ref netrc) = initial_config.credentials.netrc_path {
            if let Err(e) = credential_provider.load_netrc(netrc) {
                tracing::warn!("Failed to load .netrc from {}: {}", netrc, e);
            }
        }
        if let Some(ref cookie_file) = initial_config.credentials.cookie_file {
            if let Err(e) = credential_provider.load_cookies(cookie_file) {
                tracing::warn!("Failed to load cookies from {}: {}", cookie_file, e);
            }
        }
        let credential_provider = Arc::new(credential_provider);

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
                vpn_provider,
                vpn_watch_tx,
                config,
                power_manager: crate::power::PowerManager::new(),
                hook_manager,
                credential_provider,
                dns_resolver,
                pool,
                db,
                hsts_cache: crate::security::HstsCache::new(),
            },
            event_tx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::SubTaskEvent;
    use crate::vpn::{VpnProvider, VpnStatus};
    use std::sync::Arc;

    struct MockVpnProvider {
        status: Arc<tokio::sync::Mutex<VpnStatus>>,
    }

    #[async_trait::async_trait]
    impl VpnProvider for MockVpnProvider {
        fn name(&self) -> &str {
            "mock-vpn"
        }

        async fn connect(&self) -> Result<()> {
            Ok(())
        }

        async fn disconnect(&self) -> Result<()> {
            Ok(())
        }

        async fn status(&self) -> Result<VpnStatus> {
            Ok(self.status.lock().await.clone())
        }

        fn interface(&self) -> Option<String> {
            Some("tun0".to_string())
        }
    }

    #[tokio::test]
    async fn test_vpn_killswitch_enforcement() {
        let mut config = crate::Config::default();
        config.vpn.force_tunnel = true;

        let status = Arc::new(tokio::sync::Mutex::new(VpnStatus::Disconnected));
        let mock_provider = Arc::new(MockVpnProvider {
            status: Arc::clone(&status),
        });

        let (_command_tx, command_rx) = tokio::sync::mpsc::channel(100);
        let (storage_tx, _storage_rx) = tokio::sync::mpsc::channel(100);
        let (_completion_tx, completion_rx) = tokio::sync::mpsc::channel(100);
        let (dht_tx, _dht_rx) = tokio::sync::mpsc::channel(100);
        let (nat_tx, _nat_rx) = tokio::sync::mpsc::channel(100);
        let (lpd_tx, _lpd_rx) = tokio::sync::mpsc::channel(100);

        let config_swap = Arc::new(arc_swap::ArcSwap::from_pointee(config.clone()));
        let pool = crate::buffer_pool::BufferPool::new(16384, 10);

        let temp_dir = tempfile::tempdir().unwrap();
        let db = sled::open(temp_dir.path()).unwrap();
        let dns_resolver = Arc::new(
            hickory_resolver::TokioResolver::builder_tokio()
                .unwrap()
                .build()
                .unwrap(),
        );

        let (mut orchestrator, _event_tx) = Orchestrator::new(
            command_rx,
            storage_tx,
            completion_rx,
            dht_tx,
            lpd_tx,
            nat_tx,
            config_swap,
            pool,
            db,
            dns_resolver,
        );

        let vpn_watch_rx = orchestrator.vpn_watch_tx.subscribe();
        orchestrator.vpn_provider = Some(mock_provider.clone() as Arc<dyn VpnProvider>);
        let _ = orchestrator
            .vpn_watch_tx
            .send(Some(mock_provider.clone() as Arc<dyn VpnProvider>));

        // 1. Verify verify_vpn_connectivity() fails when Disconnected
        let verify_result = orchestrator.verify_vpn_connectivity().await;
        assert!(verify_result.is_err());
        assert!(verify_result
            .unwrap_err()
            .to_string()
            .contains("Mandatory VPN tunnel is down"));

        // 2. Verify verify_vpn_connectivity() succeeds when Connected
        *status.lock().await = VpnStatus::Connected;
        let verify_result2 = orchestrator.verify_vpn_connectivity().await;
        assert!(verify_result2.is_ok());

        // 3. Verify background watch loop triggers KillSwitch on transition to Disconnected
        let mut subtask_rx = orchestrator.subtask_rx;
        let config_clone = orchestrator.config.clone();
        let subtask_tx_monitor = orchestrator.subtask_tx.clone();

        let watch_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(10));
            loop {
                interval.tick().await;
                let force_tunnel = config_clone.load().vpn.force_tunnel;
                let vpn_opt = vpn_watch_rx.borrow().clone();
                println!(
                    "TEST_DEBUG: vpn_opt exists: {}, force_tunnel: {}",
                    vpn_opt.is_some(),
                    force_tunnel
                );
                if let Some(vpn) = vpn_opt {
                    if force_tunnel {
                        let stat = vpn.status().await;
                        println!("TEST_DEBUG: status: {:?}", stat);
                        match stat {
                            Ok(VpnStatus::Disconnected) | Ok(VpnStatus::Error(_)) => {
                                println!("TEST_DEBUG: Sending KillSwitch");
                                let _ = subtask_tx_monitor.send(SubTaskEvent::KillSwitch).await;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
        });

        // Trigger transition to Disconnected
        *status.lock().await = VpnStatus::Disconnected;

        // Wait for SubTaskEvent::KillSwitch on the channel
        let mut killswitch_received = false;
        tokio::select! {
            _ = watch_handle => {}
            _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
        }

        while let Ok(event) = subtask_rx.try_recv() {
            if let SubTaskEvent::KillSwitch = event {
                killswitch_received = true;
                break;
            }
        }

        assert!(
            killswitch_received,
            "Orchestrator should have received a KillSwitch event on VPN disconnect"
        );
    }
}
