pub mod lifecycle_ext;
pub mod peer_handler;

pub use peer_handler::handle_incoming_peer;

use super::{Orchestrator, SubTaskEvent};
use crate::bitfield::Bitfield;
use crate::bt_task::BtTask;
use crate::task::TaskType;
use crate::worker::Metadata;
use crate::{Error, Result, TaskId};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

impl Orchestrator {
    pub(crate) async fn save_task(&self, id: TaskId) -> Result<()> {
        if let Some(meta_task) = self.tasks.get(&id) {
            let mut bitfield: Option<Bitfield> = None;
            for sub in &meta_task.subtasks {
                if sub.task_type == TaskType::BitTorrent {
                    if let Some(bt) = self.bt_tasks.get(&sub.id) {
                        let bf: tokio::sync::MutexGuard<Option<crate::bitfield::Bitfield>> =
                            bt.state.bitfield.lock().await;
                        bitfield = bf.clone();
                        break;
                    }
                }
            }

            // Save bitfield to Sled for high-performance resumption
            if let Some(ref bf) = bitfield {
                let bytes = bf.as_bytes();
                let _ = self.db.insert(format!("task:{}:bitfield", id.0), bytes);
            }

            // Save full task state to .aura control file for human-readability and basic resumption
            let state = meta_task.to_state(bitfield);
            if let Ok(json) = serde_json::to_vec_pretty(&state) {
                let config = self.config.load();
                let path = std::path::Path::new(&config.storage.download_dir)
                    .join(format!("{}.aura", meta_task.name));
                if let Err(e) = std::fs::write(&path, json) {
                    warn!(%id, ?path, error = %e, "Failed to write control file");
                }
            }
        }
        Ok(())
    }

    pub(crate) async fn start_task_loops_with_bitfield(
        &mut self,
        id: TaskId,
        token: CancellationToken,
        bitfield: Option<Bitfield>,
    ) -> Result<()> {
        let meta_task = self
            .tasks
            .get(&id)
            .ok_or_else(|| Error::Config("Task not found".to_string()))?;
        let subtasks = meta_task.subtasks.clone();
        let my_peer_id = self.peer_id;
        let local_addr = self.resolve_local_addr();
        let config_arc = self.config.clone();
        let throttler_arc = self.throttler.clone();

        for sub_task in subtasks {
            let sub_id = sub_task.id;
            let uri = sub_task.uri.clone();
            let ttype = sub_task.task_type.clone();
            let subtask_tx = self.subtask_tx.clone();
            let storage_tx = self.storage_tx.clone();
            let dht_tx = self.dht_tx.clone();
            let lpd_tx = self.lpd_tx.clone();
            let token_clone = token.clone();
            let loaded_bf = bitfield.clone();
            let config_clone = config_arc.clone();
            let db = self.db.clone();
            let throttler_clone = throttler_arc.clone();
            let provider_clone = self.credential_provider.clone();
            let dns_resolver = self.dns_resolver.clone();
            let hsts_cache = self.hsts_cache.clone();

            let existing_bt = self.bt_tasks.get(&sub_id).cloned();

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
                            .credential_provider(provider_clone.clone())
                            .hsts_cache(hsts_cache)
                            .build_http();
                        match worker.resolve_metadata().await {
                            Ok(m) => {
                                let _ = subtask_tx.send(SubTaskEvent::Matured(id, sub_id, m)).await;
                            }
                            Err(e) => {
                                let _ = subtask_tx
                                    .send(crate::orchestrator::SubTaskEvent::Failed(
                                        id,
                                        sub_id,
                                        e.to_string(),
                                    ))
                                    .await;
                            }
                        }
                    }
                    TaskType::Ftp => {
                        let worker = crate::worker::WorkerBuilder::new(uri)
                            .local_addr(local_addr)
                            .credential_provider(provider_clone.clone())
                            .build_ftp();
                        match worker.resolve_metadata().await {
                            Ok(m) => {
                                let _ = subtask_tx.send(SubTaskEvent::Matured(id, sub_id, m)).await;
                            }
                            Err(e) => {
                                let _ = subtask_tx
                                    .send(crate::orchestrator::SubTaskEvent::Failed(
                                        id,
                                        sub_id,
                                        e.to_string(),
                                    ))
                                    .await;
                            }
                        }
                    }
                    TaskType::BitTorrent => {
                        let bt_task = if let Some(bt) = existing_bt {
                            bt
                        } else {
                            let init_res = if uri.starts_with("magnet:") {
                                match crate::magnet::Magnet::parse(&uri) {
                                    Ok(m) => Ok(BtTask::from_magnet(
                                        id,
                                        m.info_hash,
                                        dht_tx,
                                        lpd_tx,
                                        db.clone(),
                                    )),
                                    Err(e) => Err(e),
                                }
                            } else {
                                BtTask::from_file(id, &uri, dht_tx, lpd_tx, db.clone()).await
                            };

                            match init_res {
                                Ok(t) => {
                                    if let Some(bf) = loaded_bf {
                                        let mut my_bf = t.state.bitfield.lock().await;
                                        *my_bf = Some(bf.clone());
                                        let mut picker = t.state.picker.lock().await;
                                        if let Some(ref mut p) = *picker {
                                            for i in 0..bf.len() {
                                                if bf.get(i) {
                                                    p.mark_completed(i);
                                                }
                                            }
                                        }
                                    }
                                    Arc::new(t)
                                }
                                Err(e) => {
                                    let _ = subtask_tx
                                        .send(crate::orchestrator::SubTaskEvent::Failed(
                                            id,
                                            sub_id,
                                            e.to_string(),
                                        ))
                                        .await;
                                    return;
                                }
                            }
                        };

                        let (worker_cmd_tx, _) = tokio::sync::broadcast::channel(1024);

                        let info_hash = bt_task.state.info_hash;
                        let _ = subtask_tx
                            .send(SubTaskEvent::BtTaskRegistered(
                                sub_id,
                                info_hash,
                                bt_task.clone(),
                                worker_cmd_tx.clone(),
                            ))
                            .await;

                        let torrent_guard = bt_task.state.torrent.lock().await;
                        if let Some(ref torrent) = *torrent_guard {
                            let total_length = torrent.total_length();
                            let metadata = Metadata {
                                final_uri: uri.clone(),
                                total_length: Some(total_length),
                                name: Some(torrent.info.name.clone()),
                            };
                            let _ = subtask_tx
                                .send(SubTaskEvent::Matured(id, sub_id, metadata))
                                .await;
                        }

                        let bt_task_run = bt_task.clone();
                        let throttler_clone = throttler_clone.clone();
                        let worker_cmd_tx_clone = worker_cmd_tx.clone();
                        tokio::spawn(async move {
                            if let Err(e) = bt_task_run
                                .run(
                                    my_peer_id,
                                    storage_tx,
                                    subtask_tx,
                                    token_clone,
                                    throttler_clone,
                                    worker_cmd_tx_clone,
                                )
                                .await
                            {
                                info!(%id, %sub_id, error = %e, "BT task failed");
                            }
                        });
                    }
                }
            });
        }
        Ok(())
    }
}
