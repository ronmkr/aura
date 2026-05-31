use super::Orchestrator;

impl Orchestrator {
    pub(crate) async fn perform_adaptive_scaling(&mut self) {
        let config = self.config.load();
        let max_concurrency = config.bandwidth.max_connections_per_task;
        let min_concurrency = config
            .bandwidth
            .min_connections_per_task
            .min(max_concurrency);

        // EWMA factor
        let alpha = 0.3;

        let mut to_dispatch = Vec::new();

        for task in self.tasks.values_mut() {
            if task.phase != crate::task::DownloadPhase::Downloading {
                continue;
            }

            for sub_task in task.subtasks.iter_mut() {
                if !sub_task.active {
                    continue;
                }

                // Calculate throughput for the last second
                let current_throughput = sub_task.recent_bytes_downloaded as f64;
                sub_task.recent_bytes_downloaded = 0;

                // Update EWMA
                if sub_task.ewma_throughput == 0.0 {
                    sub_task.ewma_throughput = current_throughput;
                } else {
                    sub_task.ewma_throughput =
                        (alpha * current_throughput) + ((1.0 - alpha) * sub_task.ewma_throughput);
                }

                // Adaptive Scaling Logic
                // If throughput per connection is low (< 256 KB/s) and we haven't reached max_concurrency, scale up.
                let throughput_per_connection = if sub_task.target_concurrency > 0 {
                    sub_task.ewma_throughput / sub_task.target_concurrency as f64
                } else {
                    0.0
                };

                // Enforce minimum connections per task limit
                if sub_task.target_concurrency < min_concurrency {
                    sub_task.target_concurrency = min_concurrency;
                    tracing::debug!(
                        meta_id = %task.id,
                        sub_id = %sub_task.id,
                        target = %sub_task.target_concurrency,
                        "Clamping subtask concurrency to minimum"
                    );
                    to_dispatch.push((task.id, sub_task.id));
                } else if sub_task.assigned_ranges.len() < sub_task.target_concurrency {
                    to_dispatch.push((task.id, sub_task.id));
                } else if (sub_task.target_concurrency < 16
                    || throughput_per_connection < 2048.0 * 1024.0)
                    && sub_task.target_concurrency < max_concurrency
                {
                    sub_task.target_concurrency =
                        (sub_task.target_concurrency + 1).clamp(min_concurrency, max_concurrency);
                    tracing::debug!(
                        meta_id = %task.id,
                        sub_id = %sub_task.id,
                        target = %sub_task.target_concurrency,
                        throughput = %sub_task.ewma_throughput,
                        "Scaling up subtask concurrency to optimize throughput"
                    );

                    to_dispatch.push((task.id, sub_task.id));
                }
            }
        }

        for (meta_id, sub_id) in to_dispatch {
            // Re-dispatch to spawn new workers immediately
            let _ = self.dispatch_next_ranges(meta_id, sub_id).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::orchestrator::{MappingEngine, Orchestrator, ResourceMappingConfig};
    use crate::task::{DownloadPhase, MetaTask, SubTask};
    use crate::{Config, TaskId};
    use arc_swap::ArcSwap;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_adaptive_scaling_min_connections() {
        let (_command_tx, command_rx) = tokio::sync::mpsc::channel(10);
        let (storage_tx, _storage_rx) = tokio::sync::mpsc::channel(10);
        let (_storage_completion_tx, storage_completion_rx) = tokio::sync::mpsc::channel(10);
        let (subtask_tx, subtask_rx) = tokio::sync::mpsc::channel(10);
        let (dht_tx, _dht_rx) = tokio::sync::mpsc::channel(10);
        let (lpd_tx, _lpd_rx) = tokio::sync::mpsc::channel(10);
        let (scrub_tx, _scrub_rx) = tokio::sync::mpsc::channel(10);
        let (nat_tx, _nat_rx) = tokio::sync::mpsc::channel(10);

        let mut config = Config::default();
        config.bandwidth.max_connections_per_task = 32;
        config.bandwidth.min_connections_per_task = 12; // Test threshold

        let config_swap = Arc::new(ArcSwap::from_pointee(config));

        let resolver_config = crate::config::ResolverConfig::default();
        let dns_resolver = Arc::new(
            crate::net_util::create_resolver(&resolver_config)
                .await
                .unwrap(),
        );

        let mut orchestrator = Orchestrator {
            tasks: std::collections::HashMap::new(),
            tenants: std::collections::HashMap::new(),
            bt_registry: std::collections::HashMap::new(),
            mapping_engine: MappingEngine::new(ResourceMappingConfig::default()),
            worker_command_txs: std::collections::HashMap::new(),
            cancellation_tokens: std::collections::HashMap::new(),
            worker_cancellation_tokens: std::collections::HashMap::new(),
            command_rx,
            event_tx: tokio::sync::broadcast::channel(10).0,
            storage_tx,
            storage_completion_rx,
            subtask_tx,
            subtask_rx,
            dht_tx,
            lpd_tx,
            scrub_tx,
            scrub_rx: None,
            _nat_tx: nat_tx,
            peer_id: [0u8; 20],
            throttler: Arc::new(crate::throttler::Throttler::new(0, 0)),
            vpn_provider: None,
            vpn_watch_tx: tokio::sync::watch::channel(None).0,
            config: config_swap,
            power_manager: crate::power::PowerManager::new(),
            _hook_service: crate::hooks::HookManager::boot(
                tokio::sync::broadcast::channel(10).1,
                crate::config::HookConfig::default(),
                crate::hooks::ShellExecutor::new(),
                crate::hooks::HookOptions::default(),
            ),
            credential_provider: Arc::new(crate::config::credentials::CredentialProvider::new()),
            dns_resolver,
            db: sled::Config::new().temporary(true).open().unwrap(),
            hsts_cache: crate::security::HstsCache::new(),
        };

        let sub_task = SubTask {
            id: TaskId(11),
            uri: "http://example.com/file".to_string(),
            task_type: crate::task::TaskType::Http,
            phase: DownloadPhase::Downloading,
            total_length: 1000,
            completed_length: 0,
            assigned_ranges: Vec::new(),
            target_concurrency: 4, // Below min_connections_per_task
            recent_bytes_downloaded: 100,
            ewma_throughput: 100.0,
            active: true,
            retry_count: 0,
        };

        let task = MetaTask {
            id: TaskId(1),
            tenant_id: None,
            name: "test".to_string(),
            phase: DownloadPhase::Downloading,
            total_length: 1000,
            completed_length: 0,
            uploaded_length: 0,
            priority: 100,
            streaming_mode: false,
            range_supported: true,
            follow_on: None,
            subtasks: vec![sub_task],
            pending_ranges: Vec::new(),
            in_flight_ranges: Vec::new(),
            checksum: None,
            seeding_start_time: None,
            blacklisted_uris: Vec::new(),
            extensions: std::collections::HashMap::new(),
            depends_on: Vec::new(),
        };

        orchestrator.tasks.insert(TaskId(1), task);

        // Run scaling
        orchestrator.perform_adaptive_scaling().await;

        // Assert target concurrency clamped to min
        let scaled_task = orchestrator.tasks.get(&TaskId(1)).unwrap();
        assert_eq!(scaled_task.subtasks[0].target_concurrency, 12);
    }
}
