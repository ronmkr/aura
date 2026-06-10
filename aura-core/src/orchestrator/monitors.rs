use crate::orchestrator::Orchestrator;
use crate::task::DownloadPhase;
use crate::TaskId;

impl Orchestrator {
    pub(crate) async fn perform_adaptive_scaling(&mut self) {
        let config = self.config.load();
        let max_concurrency = config.bandwidth.max_connections_per_task;
        let min_concurrency = config.bandwidth.min_connections_per_task;

        let ids: Vec<TaskId> = self.tasks.keys().cloned().collect();

        for id in ids {
            if let Some(task) = self.tasks.get_mut(&id) {
                if task.phase != DownloadPhase::Downloading {
                    continue;
                }

                for sub in &mut task.subtasks {
                    if sub.ewma_throughput < config.bandwidth.adaptive_scaling_low_throughput {
                        // Slow source, scale up
                        sub.target_concurrency =
                            std::cmp::min(sub.target_concurrency + 2, max_concurrency);
                    } else if sub.ewma_throughput
                        > config.bandwidth.adaptive_scaling_high_throughput
                    {
                        // Very fast source, scale down to save resources
                        sub.target_concurrency = std::cmp::max(
                            sub.target_concurrency.saturating_sub(1),
                            min_concurrency,
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "monitors_tests.rs"]
mod tests;
