use crate::bt_task::BtTask;
use crate::bt_worker::BtWorker;
use crate::orchestrator::{SubTaskEvent, WorkerCommand};
use crate::storage::StorageRequest;
use crate::{InfoHash, Result, TaskId};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

#[allow(clippy::too_many_arguments)]
pub async fn handle_incoming_peer(
    mut stream: TcpStream,
    addr: std::net::SocketAddr,
    bt_registry: std::collections::HashMap<InfoHash, Arc<BtTask>>,
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
    pool: crate::buffer_pool::BufferPool,
) -> Result<()> {
    use crate::bt_worker::Handshake;
    use crate::bt_worker::HANDSHAKE_LEN;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = [0u8; HANDSHAKE_LEN];
    stream.read_exact(&mut buf).await?;
    let handshake = Handshake::deserialize(&buf)?;

    // Find the task by matching the 20-byte hash from handshake
    let mut task_found = None;
    for (info_hash, bt_task) in &bt_registry {
        if info_hash.matches_handshake(&handshake.info_hash) {
            task_found = Some((*info_hash, bt_task.clone()));
            break;
        }
    }

    if let Some((target_info_hash, task)) = task_found {
        if let Some(token) = cancellation_tokens.get(&task.id) {
            if token.is_cancelled() {
                return Ok(());
            }

            info!(?addr, "Accepted incoming peer for task {}", task.id);

            let my_handshake = Handshake::new(handshake.info_hash, my_peer_id);
            stream.write_all(&my_handshake.serialize()).await?;

            let mut worker = BtWorker::new(
                addr.to_string(),
                target_info_hash,
                handshake.peer_id,
                my_peer_id,
                pool.clone(),
                config.network.proxy.clone(),
            );
            worker.local_addr = local_addr;
            worker.pipeline_size = config.bittorrent.request_pipeline_size;

            let w_cmd_rx = if let Some(tx) = worker_command_txs.get(&task.id) {
                tx.subscribe()
            } else {
                let (dummy_tx, _) = tokio::sync::broadcast::channel::<WorkerCommand>(1024);
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
