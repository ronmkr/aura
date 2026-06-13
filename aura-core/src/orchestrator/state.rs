use super::mapping::MappingEngine;
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

/// Channels required for Orchestrator initialization.
pub struct OrchestratorChannels {
    pub command_rx: mpsc::Receiver<Command>,
    pub storage_tx: mpsc::Sender<crate::storage::StorageRequest>,
    pub storage_completion_rx: mpsc::Receiver<crate::storage::StorageEvent>,
    pub dht_tx: mpsc::Sender<DhtCommand>,
    pub lpd_tx: mpsc::Sender<crate::lpd::LpdCommand>,
    pub nat_tx: mpsc::Sender<NatCommand>,
}

#[derive(Clone)]
pub struct OrchestratorHandle {
    pub config: Arc<ArcSwap<crate::Config>>,
    pub dns_resolver: Arc<crate::net_util::TokioResolver>,
    pub credential_provider: Arc<crate::config::credentials::CredentialProvider>,
    pub hsts_cache: crate::security::HstsCache,
    pub alt_svc_cache: crate::security::AltSvcCache,
    pub resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
    pub client_pool: crate::worker::http::ClientPool,
    pub peer_id: [u8; 20],
    pub db: sled::Db,
    pub vpn_provider: Option<Arc<dyn crate::vpn::VpnProvider>>,
}

impl OrchestratorHandle {
    pub fn build_worker_builder(
        &self,
        uri: String,
        tenant_id: Option<TenantId>,
    ) -> crate::worker::WorkerBuilder {
        let config = self.config.load();
        let local_addr = self.resolve_local_addr();
        crate::worker::WorkerBuilder::new(uri)
            .local_addr(local_addr)
            .dns_resolver(self.dns_resolver.clone())
            .user_agent(Some(config.network.user_agent.clone()))
            .connect_timeout(Some(config.network.connect_timeout_secs))
            .tcp_keepalive_secs(Some(config.network.tcp_keepalive_secs))
            .proxy(config.network.proxy.clone())
            .max_redirects(config.network.max_redirects)
            .retry_count(config.network.http_retry_count)
            .retry_delay_secs(config.network.http_retry_delay_secs)
            .happy_eyeballs_stagger_ms(config.network.happy_eyeballs_stagger_ms)
            .http_buffer_capacity(config.network.http_buffer_capacity)
            .http_concurrent_requests(config.network.http_concurrent_requests)
            .credential_provider(self.credential_provider.clone())
            .hsts_cache(self.hsts_cache.clone())
            .alt_svc_cache(self.alt_svc_cache.clone())
            .resource_governor(self.resource_governor.clone())
            .tenant_id(tenant_id)
            .client_pool(self.client_pool.clone())
    }

    pub fn build_bt_worker_options(
        &self,
        peer_addr: String,
        info_hash: InfoHash,
        peer_id: [u8; 20],
        throttler: Arc<Throttler>,
    ) -> crate::worker::bittorrent::BtWorkerOptions {
        let config = self.config.load();
        crate::worker::bittorrent::BtWorkerOptions {
            peer_addr,
            info_hash,
            peer_id,
            my_id: self.peer_id,
            proxy: config.network.proxy.clone(),
            throttler,
            pex_enabled: config.bittorrent.pex_enabled,
            pipeline_size: config.bittorrent.request_pipeline_size,
            connect_timeout_secs: config.network.connect_timeout_secs,
            happy_eyeballs_stagger_ms: config.network.happy_eyeballs_stagger_ms,
            encryption: config.bittorrent.encryption,
        }
    }
}

pub struct Orchestrator {
    pub(crate) tasks: HashMap<TaskId, MetaTask>,
    pub(crate) tenants: HashMap<TenantId, TenantContext>,
    pub(crate) bt_registry: HashMap<InfoHash, TaskId>,
    pub(crate) mapping_engine: MappingEngine,
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
    pub(crate) resource_governor: Arc<crate::orchestrator::resource_governor::ResourceGovernor>,
    pub(crate) power_manager: crate::power::PowerManager,
    pub(crate) _hook_service: crate::hooks::HookServiceHandle,
    pub(crate) credential_provider: Arc<crate::config::credentials::CredentialProvider>,
    pub(crate) dns_resolver: Arc<crate::net_util::TokioResolver>,
    pub(crate) db: sled::Db,
    pub(crate) hsts_cache: crate::security::HstsCache,
    pub(crate) alt_svc_cache: crate::security::AltSvcCache,
    pub(crate) policy_manager: crate::orchestrator::policy_manager::PolicyManager,
    pub(crate) client_pool: crate::worker::http::ClientPool,
    pub(crate) notification_service: Arc<super::notifications::NotificationService>,
}

impl Orchestrator {
    pub(crate) fn get_bt_task(&self, id: TaskId) -> Option<Arc<BtTask>> {
        // Try as meta_id
        if let Some(task) = self.tasks.get(&id) {
            if let Some(ext) = task
                .extensions
                .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
            {
                return ext.clone().as_any_arc().downcast::<BtTask>().ok();
            }
        }
        // Try as sub_id
        for task in self.tasks.values() {
            if task.subtasks.iter().any(|s| s.id == id) {
                if let Some(ext) = task
                    .extensions
                    .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
                {
                    return ext.clone().as_any_arc().downcast::<BtTask>().ok();
                }
            }
        }
        None
    }

    pub(crate) fn iter_bt_tasks(&self) -> Vec<(TaskId, Arc<BtTask>)> {
        let mut results = Vec::new();
        for task in self.tasks.values() {
            if let Some(ext) = task
                .extensions
                .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
            {
                if let Ok(bt) = ext.clone().as_any_arc().downcast::<BtTask>() {
                    results.push((task.id, bt));
                }
            }
        }
        results
    }

    pub(crate) fn resolve_base_dir(&self, tenant_id: &Option<TenantId>) -> std::path::PathBuf {
        let config = self.config.load();
        if let Some(ref tid) = tenant_id {
            if let Some(ctx) = self.tenants.get(tid) {
                if let Some(ref root) = ctx.disk_path_root {
                    return root.clone();
                }
            }
        }
        std::path::PathBuf::from(&config.storage.download_dir)
    }

    pub(crate) fn resolve_throttler(&self, tenant_id: &Option<TenantId>) -> Arc<Throttler> {
        if let Some(ref tid) = tenant_id {
            if let Some(ctx) = self.tenants.get(tid) {
                return ctx.throttler.clone();
            }
        }
        self.throttler.clone()
    }

    pub(crate) fn handle(&self) -> OrchestratorHandle {
        OrchestratorHandle {
            config: self.config.clone(),
            dns_resolver: self.dns_resolver.clone(),
            credential_provider: self.credential_provider.clone(),
            hsts_cache: self.hsts_cache.clone(),
            alt_svc_cache: self.alt_svc_cache.clone(),
            resource_governor: self.resource_governor.clone(),
            client_pool: self.client_pool.clone(),
            peer_id: self.peer_id,
            db: self.db.clone(),
            vpn_provider: self.vpn_provider.clone(),
        }
    }

    pub(crate) fn emit_progress(&self, id: TaskId) {
        if let Some(task) = self.tasks.get(&id) {
            let _ = self.event_tx.send(Event::TaskProgress {
                id,
                completed_bytes: task.completed_length,
                uploaded_bytes: task.uploaded_length(),
                total_bytes: task.total_length,
            });
        }
    }

    pub fn new(
        channels: OrchestratorChannels,
        config: Arc<ArcSwap<crate::Config>>,
        db: sled::Db,
        dns_resolver: Arc<crate::net_util::TokioResolver>,
    ) -> (Self, broadcast::Sender<Event>) {
        let initial_config = config.load();
        let (event_tx, _event_rx) =
            broadcast::channel(initial_config.limits.event_channel_capacity);
        let (subtask_tx, subtask_rx) =
            mpsc::channel(initial_config.limits.event_channel_capacity * 4);
        let (scrub_tx, scrub_rx) = mpsc::channel(initial_config.limits.command_channel_capacity);

        let mut peer_id = [0u8; 20];
        let prefix = initial_config.bittorrent.peer_id_prefix.as_bytes();
        let prefix_len = prefix.len().min(8);
        peer_id[..prefix_len].copy_from_slice(&prefix[..prefix_len]);
        rand::rng().fill(&mut peer_id[prefix_len..]);

        let initial_config = config.load();
        let throttler = Arc::new(Throttler::new(
            initial_config.bandwidth.global_download_limit,
            initial_config.bandwidth.global_upload_limit,
            initial_config.bandwidth.refill_interval_ms,
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

        let limit_bytes = (initial_config.storage.memory_limit_mb as usize) * 1024 * 1024;
        let safety_margin_bytes =
            (initial_config.storage.memory_safety_margin_mb as usize) * 1024 * 1024;
        let resource_governor = Arc::new(
            crate::orchestrator::resource_governor::ResourceGovernor::new(
                limit_bytes,
                safety_margin_bytes,
            ),
        );

        (
            Self {
                tasks: HashMap::new(),
                tenants: HashMap::new(),
                bt_registry: HashMap::new(),
                mapping_engine: MappingEngine::new(initial_config.resource_mapping.clone()),
                worker_command_txs: HashMap::new(),
                cancellation_tokens: HashMap::new(),
                worker_cancellation_tokens: HashMap::new(),
                command_rx: channels.command_rx,
                event_tx: event_tx.clone(),
                storage_tx: channels.storage_tx,
                storage_completion_rx: channels.storage_completion_rx,
                subtask_tx,
                subtask_rx,
                scrub_tx,
                scrub_rx: Some(scrub_rx),
                dht_tx: channels.dht_tx,
                lpd_tx: channels.lpd_tx,
                _nat_tx: channels.nat_tx,
                peer_id,
                throttler,
                vpn_provider,
                vpn_watch_tx,
                config: config.clone(),
                resource_governor,
                power_manager: crate::power::PowerManager::new(),
                _hook_service: hook_service,
                credential_provider,
                dns_resolver,
                db,
                hsts_cache: crate::security::HstsCache::new(),
                alt_svc_cache: crate::security::AltSvcCache::new(),
                policy_manager: crate::orchestrator::policy_manager::PolicyManager::new(),
                client_pool: crate::worker::http::ClientPool::new(),
                notification_service: Arc::new(super::notifications::NotificationService::new(
                    config.clone(),
                )),
            },
            event_tx,
        )
    }
}
