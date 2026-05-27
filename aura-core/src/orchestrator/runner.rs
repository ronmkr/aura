use super::structs::{Orchestrator, SubTaskEvent};
use crate::{Result, TaskId};
use std::net::{IpAddr, Ipv4Addr};

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
        let mut scaling_interval = tokio::time::interval(std::time::Duration::from_secs(1));

        // VPN Kill-switch Monitor
        let vpn_watch_rx = self.vpn_watch_tx.subscribe();
        let subtask_tx_monitor = self.subtask_tx.clone();
        let config_monitor = self.config.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            loop {
                interval.tick().await;

                let force_tunnel = config_monitor.load().vpn.force_tunnel;

                // Pick up the latest provider
                let vpn_opt = vpn_watch_rx.borrow().clone();

                if let Some(vpn) = vpn_opt {
                    if force_tunnel {
                        match vpn.status().await {
                            Ok(crate::vpn::VpnStatus::Disconnected)
                            | Ok(crate::vpn::VpnStatus::Error(_)) => {
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
                            _ => {}
                        }
                    }
                }

                if subtask_tx_monitor.is_closed() {
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
                _ = save_interval.tick() => {
                    self.check_seed_limits().await;
                    let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();
                    for id in ids {
                        let _ = self.save_task(id).await;

                        // Stall detection for Scrubber (ADR 0024)
                        if let Some(task) = self.tasks.get(&id) {
                            if task.phase == crate::task::DownloadPhase::Downloading {
                                let total_throughput: f64 = task.subtasks.iter().map(|s| s.ewma_throughput).sum();
                                if total_throughput < 1.0 { // Practically 0
                                    tracing::info!(%id, "Task stalled (0 throughput). Triggering integrity scrub.");
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
                    let pool = self.pool.clone();
                    let throttler = self.throttler.clone();

                    let bt_tasks = self.bt_tasks.clone();

                    tokio::spawn(async move {
                        if let Err(e) = super::lifecycle::handle_incoming_peer(stream, addr, bt_registry, bt_tasks, worker_command_txs, storage_tx, subtask_tx, my_peer_id, cancellation_tokens, local_addr, config, pool, throttler).await {
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
