use super::lifecycle::peer_handler::IncomingPeerContext;
use super::{Orchestrator, SubTaskEvent};
use crate::worker::bittorrent::task::BtTask;
use crate::{Result, TaskId};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::Arc;

impl Orchestrator {
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
        let mut scaling_interval = tokio::time::interval(std::time::Duration::from_millis(
            config_initial.general.event_poll_interval_ms,
        ));
        let mut pex_interval = tokio::time::interval(std::time::Duration::from_secs(60));

        // VPN Kill-switch Monitor
        let vpn_watch_rx = self.vpn_watch_tx.subscribe();
        let subtask_tx_monitor = self.subtask_tx.clone();
        let config_monitor = self.config.clone();

        let vpn_check_secs = config_initial.vpn.check_interval_secs;
        tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(vpn_check_secs));
            let mut last_status = crate::vpn::VpnStatus::Disconnected;

            loop {
                interval.tick().await;

                let config = config_monitor.load();
                let force_tunnel = config.vpn.force_tunnel;
                let auto_connect = config.vpn.auto_connect;

                // Pick up the latest provider
                let vpn_opt = vpn_watch_rx.borrow().clone();

                if let Some(vpn) = vpn_opt {
                    match vpn.status().await {
                        Ok(status) => {
                            if status != last_status {
                                tracing::info!(
                                    provider = %vpn.name(),
                                    from = ?last_status,
                                    to = ?status,
                                    "VPN status transition detected"
                                );
                                last_status = status.clone();
                            }

                            match status {
                                crate::vpn::VpnStatus::Disconnected
                                | crate::vpn::VpnStatus::Error(_) => {
                                    if force_tunnel {
                                        tracing::warn!(
                                            provider = %vpn.name(),
                                            interface = ?vpn.interface(),
                                            "VPN Kill-switch triggered! Connection lost. Stopping all tasks."
                                        );
                                        if subtask_tx_monitor
                                            .send(SubTaskEvent::KillSwitch)
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }

                                    if auto_connect {
                                        tracing::info!(provider = %vpn.name(), "VPN disconnected, attempting auto-connect...");
                                        if let Err(e) = vpn.connect().await {
                                            tracing::error!(provider = %vpn.name(), error = %e, "VPN auto-connect failed");
                                        }
                                    }
                                }
                                crate::vpn::VpnStatus::Connecting => {
                                    tracing::debug!(provider = %vpn.name(), "VPN is connecting...");
                                }
                                crate::vpn::VpnStatus::Connected => {
                                    // Connection secure
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!(provider = %vpn.name(), error = %e, "Failed to query VPN status");
                        }
                    }
                }

                if subtask_tx_monitor.is_closed() {
                    break;
                }
            }
        });

        // Network Interface Roaming Reconnector Monitor
        let subtask_tx_roaming = self.subtask_tx.clone();
        tokio::spawn(async move {
            let mut last_ip = local_ip_address::local_ip().ok();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;

                let current_ip = local_ip_address::local_ip().ok();
                if current_ip != last_ip {
                    tracing::warn!(
                        ?last_ip,
                        ?current_ip,
                        "Interface Roaming detected! Active network interface IP changed."
                    );
                    last_ip = current_ip;
                    if subtask_tx_roaming
                        .send(SubTaskEvent::RoamingDetected)
                        .await
                        .is_err()
                    {
                        break;
                    }
                }

                if subtask_tx_roaming.is_closed() {
                    break;
                }
            }
        });

        if let Some(scrub_command_rx) = self.scrub_rx.take() {
            let (scrub_event_tx, mut scrub_event_rx) = tokio::sync::mpsc::channel(1024);
            let scrubber =
                crate::scrubber::IntegrityScrubber::new(scrub_command_rx, scrub_event_tx);
            tokio::spawn(scrubber.run());

            let sub_tx_clone = self.subtask_tx.clone();
            tokio::spawn(async move {
                while let Some(event) = scrub_event_rx.recv().await {
                    let _ = sub_tx_clone.send(SubTaskEvent::ScrubberEvent(event)).await;
                }
            });
        }

        loop {
            tokio::select! {
                _ = scaling_interval.tick() => {
                    self.perform_adaptive_scaling().await;
                }
                _ = pex_interval.tick() => {
                    if self.config.load().bittorrent.pex_enabled {
                        for (task_id, bt_task) in self.iter_bt_tasks() {
                            if let Some(tx) = self.worker_command_txs.get(&task_id) {
                                let active_peers: std::collections::HashSet<std::net::SocketAddr> = {
                                    let registry = bt_task.state.registry.lock().await;
                                    registry.get_connected_peers().into_iter().filter_map(|p| {
                                        format!("{}:{}", p.ip, p.port).parse().ok()
                                    }).collect()
                                };
                                let _ = tx.send(crate::orchestrator::WorkerCommand::PexUpdate(active_peers));
                            }
                        }
                    }
                }
                _ = save_interval.tick() => {
                    self.check_seed_limits().await;
                    let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                    for id in ids {
                        let _ = self.save_task(id).await;

                        // Stall detection for Scrubber (ADR 0024)
                        if let Some(task) = self.tasks.get_mut(&id) {
                            if task.phase == crate::task::DownloadPhase::Downloading {
                                let total_throughput: f64 = task.subtasks.iter().map(|s| s.ewma_throughput).sum();

                                if total_throughput < 1.0 {
                                    task.stall_ticks += 1;
                                } else {
                                    task.stall_ticks = 0;
                                }

                                // Trigger only after 6 consecutive stalls (approx 3 minutes with 30s interval)
                                if task.stall_ticks >= 6 {
                                    tracing::info!(%id, "Task stalled (3+ minutes of 0 throughput). Triggering integrity scrub.");
                                    task.stall_ticks = 0; // Reset after trigger
                                    let _ = self.handle_command(crate::orchestrator::Command::Scrub(id)).await;
                                }
                            }
                        }
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
                    let throttler = self.throttler.clone();

                    let bt_tasks: std::collections::HashMap<TaskId, Arc<BtTask>> = self.iter_bt_tasks().into_iter().collect();

                    tokio::spawn(async move {
                        let ctx = IncomingPeerContext {
                            bt_registry,
                            bt_tasks,
                            worker_command_txs,
                            storage_tx,
                            subtask_tx,
                            my_peer_id,
                            cancellation_tokens,
                            local_addr,
                            config,
                            throttler,
                        };
                        if let Err(e) = super::lifecycle::handle_incoming_peer(stream, addr, ctx).await
                        {
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
                cmd_res = self.command_rx.recv() => {
                    match cmd_res {
                        Some(cmd) => {
                            if let Err(e) = self.handle_command(cmd).await {
                                if e.to_string().contains("Shutting down") {
                                    tracing::info!("Orchestrator shutting down gracefully");
                                    return Ok(());
                                }
                                tracing::error!("Command handle error: {}", e);
                            }
                        }
                        None => {
                            tracing::warn!("Orchestrator command channel closed, exiting loop");
                            return Ok(());
                        }
                    }
                    self.update_power_management();
                }
                Some(event) = self.storage_completion_rx.recv() => {
                    if let Err(e) = self.handle_storage_event(event).await {
                        tracing::error!("Storage event error: {}", e);
                    }
                    self.update_power_management();
                }
            }
        }
    }
}
