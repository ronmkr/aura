pub mod lifecycle_ext;
pub mod peer_handler;

pub use peer_handler::handle_incoming_peer;

use super::{Orchestrator, SubTaskEvent};
use crate::bitfield::Bitfield;
use crate::bt_task::BtTask;
use crate::bt_worker::BtWorker;
use crate::task::TaskType;
use crate::worker::Metadata;
use crate::{Error, Result, TaskId};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

impl Orchestrator {
    pub(crate) async fn save_task(&self, id: TaskId) -> Result<()> {
        if let Some(meta_task) = self.tasks.get(&id) {
            let mut bitfield = None;
            for sub in &meta_task.subtasks {
                if let Some(bt) = self.bt_tasks.get(&sub.id) {
                    let bf = bt.state.bitfield.lock().await;
                    bitfield = bf.clone();
                    break;
                }
            }

            let state = meta_task.to_state(bitfield);
            let filename = format!("{}.aura", meta_task.name);
            let path = std::env::current_dir().unwrap_or_default().join(&filename);
            info!(%id, path = ?path, "Saving control file");

            let data = serde_json::to_vec_pretty(&state)
                .map_err(|e| Error::Storage(format!("Failed to serialize task state: {}", e)))?;

            tokio::fs::write(&path, data).await.map_err(|e| {
                Error::Storage(format!("Failed to write control file {}: {}", filename, e))
            })?;
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

        for sub_task in subtasks {
            let sub_id = sub_task.id;
            let uri = sub_task.uri.clone();
            let ttype = sub_task.task_type.clone();
            let subtask_tx = self.subtask_tx.clone();
            let storage_tx = self.storage_tx.clone();
            let dht_tx = self.dht_tx.clone();
            let lpd_tx = self.lpd_tx.clone();
            let token = token.clone();
            let loaded_bf = bitfield.clone();
            let config_clone = config_arc.clone();

            let existing_bt = self.bt_tasks.get(&sub_id).cloned();

            tokio::spawn(async move {
                let config = config_clone.load();
                match ttype {
                    TaskType::Http => {
                        let worker = crate::worker::WorkerBuilder::new(uri)
                            .local_addr(local_addr)
                            .user_agent(Some(config.network.user_agent.clone()))
                            .connect_timeout(Some(config.network.connect_timeout_secs))
                            .proxy(config.network.proxy.clone())
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
                    TaskType::BitTorrent => {
                        let bt_task = if let Some(bt) = existing_bt {
                            bt
                        } else {
                            let init_res = if uri.starts_with("magnet:") {
                                match crate::magnet::Magnet::parse(&uri) {
                                    Ok(m) => {
                                        Ok(BtTask::from_magnet(id, m.info_hash, dht_tx, lpd_tx))
                                    }
                                    Err(e) => Err(e),
                                }
                            } else {
                                BtTask::from_file(id, &uri, dht_tx, lpd_tx).await
                            };

                            match init_res {
                                Ok(t) => {
                                    let t = t;
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
                                        .send(SubTaskEvent::Failed(id, sub_id, e.to_string()))
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
                        drop(torrent_guard);

                        let tracker_task = bt_task.clone();
                        let t1 = token.clone();
                        let ua = config.network.user_agent.clone();
                        let port = config.network.listen_port;
                        tokio::spawn(async move {
                            let _ = tracker_task
                                .run_tracker_loop(my_peer_id, port, t1, local_addr, Some(ua))
                                .await;
                        });

                        let dht_task = bt_task.clone();
                        let t2 = token.clone();
                        tokio::spawn(async move {
                            let _ = dht_task.run_dht_loop(t2).await;
                        });

                        if config.bittorrent.lpd_enabled {
                            let lpd_task = bt_task.clone();
                            let t5 = token.clone();
                            let port = config.network.listen_port;
                            tokio::spawn(async move {
                                let _ = lpd_task.run_lpd_loop(port, t5).await;
                            });
                        }

                        let peer_task = bt_task.clone();
                        let s_tx = storage_tx.clone();
                        let s_tx_progress = subtask_tx.clone();
                        let t3 = token.clone();
                        let config_loop = config_clone.clone();

                        use std::sync::atomic::{AtomicUsize, Ordering};
                        let active_workers = Arc::new(AtomicUsize::new(0));

                        tokio::spawn(async move {
                            loop {
                                if t3.is_cancelled() {
                                    break;
                                }
                                let config = config_loop.load();
                                if active_workers.load(Ordering::Relaxed)
                                    < config.bittorrent.max_peers_per_torrent
                                {
                                    if let Some((maybe_piece_idx, peer)) =
                                        peer_task.pick_work().await
                                    {
                                        let addr = format!("{}:{}", peer.ip, peer.port);
                                        let peer_id = peer
                                            .id
                                            .and_then(|v| {
                                                if let serde_bencode::value::Value::Bytes(b) = v {
                                                    let mut pid = [0u8; 20];
                                                    if b.len() == 20 {
                                                        pid.copy_from_slice(&b);
                                                        Some(pid)
                                                    } else {
                                                        None
                                                    }
                                                } else {
                                                    None
                                                }
                                            })
                                            .unwrap_or([0; 20]);

                                        info!(%id, %addr, ?maybe_piece_idx, "Initiating peer connection");
                                        peer_task
                                            .update_peer_state(
                                                &addr,
                                                crate::peer_registry::ConnectionState::Connecting,
                                            )
                                            .await;

                                        let mut worker = BtWorker::new(
                                            addr.clone(),
                                            info_hash,
                                            peer_id,
                                            my_peer_id,
                                        );
                                        worker.local_addr = local_addr;
                                        worker.pipeline_size =
                                            config.bittorrent.request_pipeline_size;
                                        let s_tx_inner = s_tx.clone();
                                        let sub_tx = s_tx_progress.clone();
                                        let peer_task_inner = peer_task.clone();
                                        let active_counter = active_workers.clone();
                                        let t4 = t3.clone();
                                        let w_cmd_rx = worker_cmd_tx.subscribe();

                                        active_counter.fetch_add(1, Ordering::Relaxed);
                                        tokio::spawn(async move {
                                            if let Err(e) = worker
                                                .run_loop(
                                                    id,
                                                    sub_id,
                                                    peer_task_inner,
                                                    s_tx_inner,
                                                    sub_tx,
                                                    w_cmd_rx,
                                                    t4,
                                                )
                                                .await
                                            {
                                                debug!(%id, %addr, error = %e, "Peer loop stopped");
                                            }
                                            active_counter.fetch_sub(1, Ordering::Relaxed);
                                        });
                                    }
                                }
                                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                            }
                        });
                    }
                }
            });
        }
        Ok(())
    }
}
