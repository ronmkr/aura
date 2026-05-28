use super::Orchestrator;

impl Orchestrator {
    pub(crate) async fn perform_adaptive_scaling(&mut self) {
        let config = self.config.load();
        let max_concurrency = config.bandwidth.max_connections_per_task;

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

                if sub_task.assigned_ranges.len() < sub_task.target_concurrency {
                    to_dispatch.push((task.id, sub_task.id));
                } else if (sub_task.target_concurrency < 16
                    || throughput_per_connection < 2048.0 * 1024.0)
                    && sub_task.target_concurrency < max_concurrency
                {
                    sub_task.target_concurrency =
                        (sub_task.target_concurrency + 1).min(max_concurrency);
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
