use super::{Orchestrator, SubTaskEvent, WorkerCommand};
use crate::bitfield::Bitfield;
use crate::bt_task::BtTask;
use crate::bt_worker::BtWorker;
use crate::storage::StorageRequest;
use crate::task::TaskType;
use crate::worker::{Metadata, ProtocolWorker, Segment};
use crate::{Error, Result, TaskId};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_incoming_peer(
    mut stream: TcpStream,
    addr: std::net::SocketAddr,
    bt_registry: std::collections::HashMap<[u8; 20], Arc<BtTask>>,
    worker_command_txs: std::collections::HashMap<
        TaskId,
        tokio::sync::broadcast::Sender<WorkerCommand>,
    >,
    storage_tx: mpsc::Sender<StorageRequest>,
    subtask_tx: mpsc::Sender<SubTaskEvent>,
    my_peer_id: [u8; 20],
    cancellation_tokens: std::collections::HashMap<TaskId, CancellationToken>,
    local_addr: Option<std::net::IpAddr>,
    config: Arc<crate::Config>,
) -> Result<()> {
    use crate::bt_worker::Handshake;
    use crate::bt_worker::HANDSHAKE_LEN;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = [0u8; HANDSHAKE_LEN];
    stream.read_exact(&mut buf).await?;
    let handshake = Handshake::deserialize(&buf)?;

    if let Some(task) = bt_registry.get(&handshake.info_hash) {
        if let Some(token) = cancellation_tokens.get(&task.id) {
            if token.is_cancelled() {
                return Ok(());
            }

            info!(?addr, "Accepted incoming peer for task {}", task.id);

            let my_handshake = Handshake::new(handshake.info_hash, my_peer_id);
            stream.write_all(&my_handshake.serialize()).await?;

            let mut worker = BtWorker::new(
                addr.to_string(),
                handshake.info_hash,
                handshake.peer_id,
                my_peer_id,
            );
            worker.local_addr = local_addr;
            worker.pipeline_size = config.bittorrent.request_pipeline_size;

            // SubId in bt_tasks/worker_command_txs is task.id for simple single-torrent tasks
            // In more complex ones it might be different, but for now we use task.id
            let w_cmd_rx = if let Some(tx) = worker_command_txs.get(&task.id) {
                tx.subscribe()
            } else {
                // Fallback to a dummy channel if not found (shouldn't happen for valid tasks)
                let (dummy_tx, _) = tokio::sync::broadcast::channel::<WorkerCommand>(1);
                dummy_tx.subscribe()
            };

            worker
                .run_loop_with_stream(
                    stream,
                    task.id,
                    task.id,
                    task.clone(),
                    storage_tx,
                    subtask_tx,
                    w_cmd_rx,
                    token.clone(),
                )
                .await
        } else {
            Ok(())
        }
    } else {
        debug!(?addr, "Rejected incoming peer: unknown info_hash");
        Ok(())
    }
}

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
            let token = token.clone();
            let loaded_bf = bitfield.clone();
            let config_clone = config_arc.clone();

            // Try to reuse existing BT state
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
                                    Ok(m) => Ok(BtTask::from_magnet(id, m.info_hash, dht_tx)),
                                    Err(e) => Err(e),
                                }
                            } else {
                                BtTask::from_file(id, &uri, dht_tx).await
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

                        // Start tracker loop
                        let tracker_task = bt_task.clone();
                        let t1 = token.clone();
                        let ua = config.network.user_agent.clone();
                        let port = config.network.listen_port;
                        tokio::spawn(async move {
                            let _ = tracker_task
                                .run_tracker_loop(my_peer_id, port, t1, local_addr, Some(ua))
                                .await;
                        });

                        // Start DHT loop
                        let dht_task = bt_task.clone();
                        let t2 = token.clone();
                        tokio::spawn(async move {
                            let _ = dht_task.run_dht_loop(t2).await;
                        });

                        // Start peer connection loop
                        let peer_task = bt_task.clone();
                        let storage_tx_loop = storage_tx.clone();
                        let subtask_tx_loop = subtask_tx.clone();
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
                                        let s_tx = storage_tx_loop.clone();
                                        let sub_tx = subtask_tx_loop.clone();
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
                                                    s_tx,
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

    pub(crate) async fn dispatch_next_ranges(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
    ) -> Result<()> {
        let token = match self.cancellation_tokens.get(&meta_id) {
            Some(t) => t.clone(),
            None => return Ok(()),
        };

        if token.is_cancelled() {
            return Ok(());
        }

        let local_addr = self.resolve_local_addr();
        let config_arc = self.config.clone();
        let concurrency_per_subtask = 4;

        loop {
            if token.is_cancelled() {
                break;
            }

            let meta_task = self
                .tasks
                .get_mut(&meta_id)
                .ok_or_else(|| Error::Config("Task not found".to_string()))?;

            let (uri, ttype, current_concurrency) = {
                let sub_task = meta_task
                    .subtasks
                    .iter()
                    .find(|s| s.id == sub_id)
                    .ok_or_else(|| Error::Config("Subtask not found".to_string()))?;
                (
                    sub_task.uri.clone(),
                    sub_task.task_type.clone(),
                    sub_task.assigned_ranges.len(),
                )
            };

            if current_concurrency >= concurrency_per_subtask {
                break;
            }

            if let Some(range) = meta_task.pick_range_for_subtask(sub_id) {
                let storage_tx = self.storage_tx.clone();
                let subtask_tx = self.subtask_tx.clone();
                let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
                let token = token.clone();
                let config_clone = config_arc.clone();

                let subtask_tx_progress = subtask_tx.clone();
                tokio::spawn(async move {
                    while let Some(bytes) = progress_rx.recv().await {
                        let _ = subtask_tx_progress
                            .send(SubTaskEvent::Downloaded(meta_id, bytes))
                            .await;
                    }
                });

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
                            let segment = Segment {
                                offset: range.start,
                                length: range.length(),
                            };

                            tokio::select! {
                                _ = token.cancelled() => {
                                    // Range will be recycled by Orchestrator on pause
                                }
                                res = worker.fetch_segment(meta_id, segment, Some(progress_tx)) => {
                                    match res {
                                        Ok(piece) => {
                                            let _ = storage_tx.send(StorageRequest::Write {
                                                task_id: meta_id,
                                                segment: piece.segment,
                                                data: piece.data,
                                            }).await;
                                            let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, range)).await;
                                        }
                                        Err(e) => {
                                            debug!(%meta_id, %sub_id, error = %e, "Range fetch failed");
                                            let _ = subtask_tx.send(SubTaskEvent::Failed(meta_id, sub_id, e.to_string())).await;
                                        }
                                    }
                                }
                            }
                        }
                        TaskType::BitTorrent => {}
                        TaskType::Ftp => {
                            let worker = crate::worker::WorkerBuilder::new(uri)
                                .local_addr(local_addr)
                                .build_ftp();
                            let segment = Segment {
                                offset: range.start,
                                length: range.length(),
                            };

                            tokio::select! {
                                _ = token.cancelled() => {
                                }
                                res = worker.fetch_segment(meta_id, segment, Some(progress_tx)) => {
                                    match res {
                                        Ok(piece) => {
                                            let _ = storage_tx.send(StorageRequest::Write {
                                                task_id: meta_id,
                                                segment: piece.segment,
                                                data: piece.data,
                                            }).await;
                                            let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, range)).await;
                                        }
                                        Err(e) => {
                                            debug!(%meta_id, %sub_id, error = %e, "Range fetch failed");
                                            let _ = subtask_tx.send(SubTaskEvent::Failed(meta_id, sub_id, e.to_string())).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            } else {
                break;
            }
        }
        Ok(())
    }
}
