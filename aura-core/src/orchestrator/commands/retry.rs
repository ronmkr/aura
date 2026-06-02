use crate::orchestrator::{Orchestrator, SubTaskEvent};
use crate::task::{DownloadPhase, TaskType};
use crate::{Error, Result, TaskId};

impl Orchestrator {
    pub(crate) async fn handle_retry_subtask(&mut self, id: TaskId, sub_id: TaskId) -> Result<()> {
        let meta_task = self.tasks.get_mut(&id).ok_or(Error::TaskNotFound(id))?;

        let token = self
            .cancellation_tokens
            .get(&id)
            .cloned()
            .ok_or_else(|| Error::Engine("No cancellation token for task".to_string()))?;

        if token.is_cancelled() {
            return Ok(());
        }

        if let Some(sub_task) = meta_task.subtasks.iter_mut().find(|s| s.id == sub_id) {
            if sub_task.phase != DownloadPhase::Degraded || sub_task.active {
                return Ok(());
            }

            let uri = sub_task.uri.clone();
            let ttype = sub_task.task_type.clone();

            if meta_task.blacklisted_uris.contains(&uri) {
                sub_task.phase = DownloadPhase::Error;
                sub_task.active = false;
                return Ok(());
            }

            sub_task.active = true;

            let subtask_tx = self.subtask_tx.clone();
            let config_clone = self.config.clone();
            let local_addr = self.resolve_local_addr();
            let provider_clone = self.credential_provider.clone();
            let dns_resolver = self.dns_resolver.clone();
            let hsts_cache = self.hsts_cache.clone();
            let alt_svc_cache = self.alt_svc_cache.clone();

            tracing::info!(%id, %sub_id, %uri, "Retrying/Self-healing Degraded subtask");

            tokio::spawn(async move {
                let config = config_clone.load();
                match ttype {
                    TaskType::Http => {
                        let worker = crate::worker::WorkerBuilder::new(uri)
                            .local_addr(local_addr)
                            .dns_resolver(dns_resolver)
                            .user_agent(Some(config.network.user_agent.clone()))
                            .connect_timeout(Some(config.network.connect_timeout_secs))
                            .proxy(config.network.proxy.clone())
                            .retry_count(config.network.http_retry_count)
                            .retry_delay_secs(config.network.http_retry_delay_secs)
                            .credential_provider(provider_clone)
                            .hsts_cache(hsts_cache)
                            .alt_svc_cache(alt_svc_cache)
                            .build_http();
                        match worker.resolve_metadata().await {
                            Ok(m) => {
                                let _ = subtask_tx.send(SubTaskEvent::Matured(id, sub_id, m)).await;
                            }
                            Err(e) => {
                                let _ = subtask_tx
                                    .send(SubTaskEvent::Failed(id, sub_id, e.to_string()))
                                    .await;
                            }
                        }
                    }
                    TaskType::Ftp => {
                        let worker = crate::worker::WorkerBuilder::new(uri)
                            .local_addr(local_addr)
                            .credential_provider(provider_clone)
                            .build_ftp();
                        match worker.resolve_metadata().await {
                            Ok(m) => {
                                let _ = subtask_tx.send(SubTaskEvent::Matured(id, sub_id, m)).await;
                            }
                            Err(e) => {
                                let _ = subtask_tx
                                    .send(SubTaskEvent::Failed(id, sub_id, e.to_string()))
                                    .await;
                            }
                        }
                    }
                    TaskType::BitTorrent => {}
                }
            });
        }

        Ok(())
    }
}
