use super::SubTaskEvent;
use crate::orchestrator::Orchestrator;
use crate::task::Range;
use crate::worker::bittorrent::task::BtTask;
use crate::{Error, Result, TaskId};
use tokio_util::sync::CancellationToken;

impl Orchestrator {
    pub(crate) async fn dispatch_bittorrent_peer(
        &mut self,
        meta_id: TaskId,
        sub_id: TaskId,
        token: &CancellationToken,
    ) -> Result<bool> {
        let meta_task = self
            .tasks
            .get_mut(&meta_id)
            .ok_or(Error::TaskNotFound(meta_id))?;

        let worker_tx = self
            .worker_command_txs
            .get(&sub_id)
            .cloned()
            .unwrap_or_else(|| {
                let capacity = self.config.load().limits.event_channel_capacity;
                let (tx, _) = tokio::sync::broadcast::channel(capacity);
                tx
            });
        let bt_task = match meta_task
            .extensions
            .get(crate::worker::bittorrent::BT_EXTENSION_KEY)
            .and_then(|e| e.clone().as_any_arc().downcast::<BtTask>().ok())
        {
            Some(bt) => bt.clone(),
            None => return Ok(false),
        };

        let peer_opt = {
            let mut registry = bt_task.state.registry.lock().await;
            registry.get_peer_to_connect()
        };

        if let Some(peer) = peer_opt {
            let peer_addr = format!("{}:{}", peer.ip, peer.port);
            let info_hash = bt_task.state.info_hash;
            let throttler_clone = self.throttler.clone();

            let storage_client = self.storage_client.clone();
            let subtask_tx = self.subtask_tx.clone();
            let child_token = token.child_token();
            self.worker_cancellation_tokens
                .insert(sub_id, child_token.clone());
            let token_clone = child_token;

            let dummy_range = Range { start: 0, end: 0 };
            meta_task.in_flight_ranges.push((sub_id, dummy_range));
            if let Some(sub) = meta_task.subtasks.iter_mut().find(|s| s.id == sub_id) {
                sub.assigned_ranges.push(dummy_range);
            }

            tracing::debug!(%meta_id, %sub_id, %peer_addr, "Spawning worker for peer");

            let orchestrator_handle = self.handle();
            tokio::spawn(async move {
                let mut worker = crate::worker::bittorrent::BtWorker::new(
                    orchestrator_handle.build_bt_worker_options(
                        peer_addr.clone(),
                        info_hash,
                        [0; 20], // peer_id will be set during handshake
                        throttler_clone,
                    ),
                );
                worker.local_addr = orchestrator_handle.resolve_local_addr();

                tokio::select! {
                    _ = token_clone.cancelled() => {}
                    res = worker.run_loop(crate::worker::bittorrent::BtWorkerArgs {
                        meta_id,
                        sub_id,
                        task: bt_task,
                        storage_client,
                        subtask_tx: subtask_tx.clone(),
                        command_rx: worker_tx.subscribe(),
                        token: token_clone.clone(),
                    }) => {
                        if let Err(e) = res {
                            tracing::debug!(%meta_id, %sub_id, error = %e, "BtWorker failed");
                        }
                        let _ = subtask_tx.send(SubTaskEvent::RangeFinished(meta_id, sub_id, dummy_range)).await;
                    }
                }
            });
            Ok(true)
        } else {
            tracing::debug!(%meta_id, %sub_id, "peer_opt is None");
            Ok(false)
        }
    }
}
