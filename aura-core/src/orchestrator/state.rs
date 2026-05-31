use super::{Command, Event, SubTaskEvent, WorkerCommand};
use crate::dht::DhtCommand;
use crate::nat::NatCommand;
use crate::task::MetaTask;
use crate::throttler::Throttler;
use crate::worker::bittorrent::task::BtTask;
use crate::{InfoHash, TaskId, TenantId};
use arc_swap::ArcSwap;
use rand::RngExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

pub struct TenantContext {
    pub throttler: Arc<Throttler>,
    pub max_tasks: Option<usize>,
    pub disk_path_root: Option<std::path::PathBuf>,
}

pub struct Orchestrator {
    pub(crate) tasks: HashMap<TaskId, MetaTask>,
    pub(crate) tenants: HashMap<TenantId, TenantContext>,
    pub(crate) bt_registry: HashMap<InfoHash, TaskId>,
    pub(crate) worker_command_txs: HashMap<TaskId, tokio::sync::broadcast::Sender<WorkerCommand>>,
    pub(crate) cancellation_tokens: HashMap<TaskId, CancellationToken>,
    pub(crate) worker_cancellation_tokens: HashMap<TaskId, CancellationToken>,
    pub(crate) command_rx: mpsc::Receiver<Command>,
    pub(crate) event_tx: broadcast::Sender<Event>,
    pub(crate) storage_tx: mpsc::Sender<crate::storage::StorageRequest>,
    pub(crate) storage_completion_rx: mpsc::Receiver<crate::storage::StorageEvent>,
    pub(crate) subtask_tx: mpsc::Sender<SubTaskEvent>,
    pub(crate) subtask_rx: mpsc::Receiver<SubTaskEvent>,
    pub(crate) dht_tx: mpsc::Sender<DhtCommand>,
    pub(crate) lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
    pub(crate) scrub_tx: mpsc::Sender<crate::scrubber::ScrubberCommand>,
    pub(crate) scrub_rx: Option<mpsc::Receiver<crate::scrubber::ScrubberCommand>>,
    pub(crate) _nat_tx: mpsc::Sender<NatCommand>,
    pub(crate) peer_id: [u8; 20],
    pub(crate) throttler: Arc<Throttler>,
    pub(crate) vpn_provider: Option<Arc<dyn crate::vpn::VpnProvider>>,
    pub(crate) vpn_watch_tx: tokio::sync::watch::Sender<Option<Arc<dyn crate::vpn::VpnProvider>>>,
    pub(crate) config: Arc<ArcSwap<crate::Config>>,
    pub(crate) power_manager: crate::power::PowerManager,
    pub(crate) _hook_service: crate::hooks::HookServiceHandle,
    pub(crate) credential_provider: Arc<crate::config::credentials::CredentialProvider>,
    pub(crate) dns_resolver: Arc<crate::net_util::TokioResolver>,
    pub(crate) db: sled::Db,
    pub(crate) hsts_cache: crate::security::HstsCache,
}

impl Orchestrator {
    pub(crate) fn get_bt_task(&self, id: TaskId) -> Option<Arc<BtTask>> {
        // Try as meta_id
        if let Some(task) = self.tasks.get(&id) {
            if let Some(ext) = task.extensions.get("bittorrent") {
                return ext.clone().as_any_arc().downcast::<BtTask>().ok();
            }
        }
        // Try as sub_id
        for task in self.tasks.values() {
            if task.subtasks.iter().any(|s| s.id == id) {
                if let Some(ext) = task.extensions.get("bittorrent") {
                    return ext.clone().as_any_arc().downcast::<BtTask>().ok();
                }
            }
        }
        None
    }

    pub(crate) fn iter_bt_tasks(&self) -> Vec<(TaskId, Arc<BtTask>)> {
        let mut results = Vec::new();
        for task in self.tasks.values() {
            if let Some(ext) = task.extensions.get("bittorrent") {
                if let Ok(bt) = ext.clone().as_any_arc().downcast::<BtTask>() {
                    results.push((task.id, bt));
                }
            }
        }
        results
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
        db: sled::Db,
        dns_resolver: Arc<crate::net_util::TokioResolver>,
    ) -> (Self, broadcast::Sender<Event>) {
        let (event_tx, _event_rx) = broadcast::channel(1024);
        let (subtask_tx, subtask_rx) = mpsc::channel(4096);
        let (scrub_tx, scrub_rx) = mpsc::channel(1024);

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
        let hook_service = crate::hooks::HookManager::boot(
            event_tx.subscribe(),
            config.clone(),
            crate::hooks::ShellExecutor::new(),
            crate::hooks::HookOptions::default(),
        );

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
                tenants: HashMap::new(),
                bt_registry: HashMap::new(),
                worker_command_txs: HashMap::new(),
                cancellation_tokens: HashMap::new(),
                worker_cancellation_tokens: HashMap::new(),
                command_rx,
                event_tx: event_tx.clone(),
                storage_tx,
                storage_completion_rx,
                subtask_tx,
                subtask_rx,
                scrub_tx,
                scrub_rx: Some(scrub_rx),
                dht_tx,
                lpd_tx,
                _nat_tx: nat_tx,
                peer_id,
                throttler,
                vpn_provider,
                vpn_watch_tx,
                config,
                power_manager: crate::power::PowerManager::new(),
                _hook_service: hook_service,
                credential_provider,
                dns_resolver,
                db,
                hsts_cache: crate::security::HstsCache::new(),
            },
            event_tx,
        )
    }
}
