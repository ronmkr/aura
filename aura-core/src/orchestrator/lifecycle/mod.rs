pub mod dispatch;
pub mod dispatch_torrent;
pub mod lifecycle_ext;
pub mod peer_handler;
pub mod recheck_ext;

pub use peer_handler::{handle_incoming_peer, IncomingPeerContext};

use super::{Orchestrator, SubTaskEvent};
use crate::bitfield::Bitfield;
use crate::task::TaskType;
use crate::worker::bittorrent::task::BtTask;
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
                    if let Some(bt) = self.get_bt_task(sub.id) {
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
                let base_dir = self.resolve_base_dir(&meta_task.tenant_id);
                let path = base_dir.join(format!("{}.aura", meta_task.name));
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
        let throttler_arc = self.resolve_throttler(&meta_task.tenant_id);

        for sub_task in subtasks {
            let sub_id = sub_task.id;
            let uri = sub_task.uri.clone();
            let ttype = sub_task.task_type.clone();
            let subtask_tx = self.subtask_tx.clone();
            let storage_client = self.storage_client.clone();
            let dht_tx = self.dht_tx.clone();
            let lpd_tx = self.lpd_tx.clone();
            let token_clone = token.clone();
            let loaded_bf = bitfield.clone();
            let throttler_clone = throttler_arc.clone();
            let tenant_id = meta_task.tenant_id.clone();
            let selected_files = meta_task.selected_files.clone();
            let streaming_mode = meta_task.streaming_mode;
            let orchestrator_handle = self.handle();

            let existing_bt = self.get_bt_task(sub_id);

            tokio::spawn(async move {
                match ttype {
                    TaskType::Http => {
                        let worker = orchestrator_handle
                            .build_worker_builder(uri, tenant_id)
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
                        let worker = orchestrator_handle
                            .build_worker_builder(uri, tenant_id)
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
                    TaskType::S3 => {
                        let worker = orchestrator_handle
                            .build_worker_builder(uri, tenant_id)
                            .build_s3();
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
                    TaskType::GDrive => {
                        let worker = orchestrator_handle
                            .build_worker_builder(uri, tenant_id)
                            .build_gdrive();
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
                    TaskType::Nntp => {
                        #[cfg(feature = "nntp")]
                        let worker = orchestrator_handle
                            .build_worker_builder(uri, tenant_id)
                            .build_nntp();
                        let res = async {
                            #[cfg(feature = "nntp")]
                            {
                                worker.resolve_metadata().await
                            }
                            #[cfg(not(feature = "nntp"))]
                            {
                                Err(crate::Error::Protocol(
                                    "NNTP feature not enabled".to_string(),
                                ))
                            }
                        }
                        .await;
                        match res {
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
                                        crate::worker::bittorrent::task::BtTaskFromMagnetArgs {
                                            id,
                                            info_hash: m.info_hash,
                                            trackers: m.trackers.clone(),
                                            dht_tx,
                                            lpd_tx,
                                            db: orchestrator_handle.db.clone(),
                                            resource_governor: orchestrator_handle
                                                .resource_governor
                                                .clone(),
                                            tenant_id: tenant_id.clone(),
                                            config: orchestrator_handle.config.clone(),
                                            streaming_mode,
                                        },
                                    )),
                                    Err(e) => Err(e),
                                }
                            } else {
                                BtTask::from_file(
                                    crate::worker::bittorrent::task::BtTaskFromFileArgs {
                                        id,
                                        path: &uri,
                                        dht_tx,
                                        lpd_tx,
                                        db: orchestrator_handle.db.clone(),
                                        bitfield: loaded_bf.clone(),
                                        resource_governor: orchestrator_handle
                                            .resource_governor
                                            .clone(),
                                        tenant_id: tenant_id.clone(),
                                        config: orchestrator_handle.config.clone(),
                                        selected_files: selected_files.as_deref(),
                                        streaming_mode,
                                    },
                                )
                                .await
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
                                id,
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
                                range_supported: true,
                                padding_ranges: torrent
                                    .get_padding_ranges(selected_files.as_deref()),
                                etag: None,
                                last_modified: None,
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
                                    orchestrator_handle.peer_id,
                                    storage_client,
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
