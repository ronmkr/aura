use crate::orchestrator::{SubTaskEvent, WorkerCommand};
use crate::storage::StorageRequest;
use crate::worker::bittorrent::task::BtTask;
use crate::worker::bittorrent::BtWorker;
use crate::{InfoHash, Result, TaskId};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

pub struct IncomingPeerContext {
    pub bt_registry: std::collections::HashMap<InfoHash, TaskId>,
    pub bt_tasks: std::collections::HashMap<TaskId, Arc<BtTask>>,
    pub worker_command_txs:
        std::collections::HashMap<TaskId, tokio::sync::broadcast::Sender<WorkerCommand>>,
    pub storage_tx: mpsc::Sender<StorageRequest>,
    pub subtask_tx: mpsc::Sender<SubTaskEvent>,
    pub my_peer_id: [u8; 20],
    pub cancellation_tokens: std::collections::HashMap<TaskId, CancellationToken>,
    pub local_addr: Option<std::net::IpAddr>,
    pub config: Arc<crate::Config>,
    pub throttler: Arc<crate::throttler::Throttler>,
}

pub async fn handle_incoming_peer(
    mut stream: TcpStream,
    addr: std::net::SocketAddr,
    ctx: IncomingPeerContext,
) -> Result<()> {
    use crate::worker::bittorrent::Handshake;
    use crate::worker::bittorrent::HANDSHAKE_LEN;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut buf = [0u8; HANDSHAKE_LEN];
    stream.read_exact(&mut buf).await?;
    let handshake = Handshake::deserialize(&buf)?;

    // Find the task by matching the 20-byte hash from handshake
    let mut task_found = None;
    for (info_hash, meta_id) in &ctx.bt_registry {
        if info_hash.matches_handshake(&handshake.info_hash) {
            if let Some(task) = ctx.bt_tasks.get(meta_id) {
                task_found = Some((*info_hash, task.clone()));
                break;
            }
        }
    }

    if let Some((target_info_hash, task)) = task_found {
        if let Some(token) = ctx.cancellation_tokens.get(&task.id) {
            if token.is_cancelled() {
                return Ok(());
            }

            info!(?addr, "Accepted incoming peer for task {}", task.id);

            let my_handshake = Handshake::new(handshake.info_hash, ctx.my_peer_id);
            stream.write_all(&my_handshake.serialize()).await?;

            let mut worker = BtWorker::new(crate::worker::bittorrent::BtWorkerOptions {
                peer_addr: addr.to_string(),
                info_hash: target_info_hash,
                peer_id: handshake.peer_id,
                my_id: ctx.my_peer_id,
                proxy: ctx.config.network.proxy.clone(),
                throttler: ctx.throttler,
                pex_enabled: ctx.config.bittorrent.pex_enabled,
                pipeline_size: ctx.config.bittorrent.request_pipeline_size,
                connect_timeout_secs: ctx.config.network.connect_timeout_secs,
                happy_eyeballs_stagger_ms: ctx.config.network.happy_eyeballs_stagger_ms,
            });
            worker.local_addr = ctx.local_addr;

            let w_cmd_rx = if let Some(tx) = ctx.worker_command_txs.get(&task.id) {
                tx.subscribe()
            } else {
                let (dummy_tx, _) = tokio::sync::broadcast::channel::<WorkerCommand>(1024);
                dummy_tx.subscribe()
            };

            worker
                .run_loop_with_stream(
                    stream,
                    crate::worker::bittorrent::BtWorkerArgs {
                        meta_id: task.id,
                        sub_id: task.id,
                        task: task.clone(),
                        storage_tx: ctx.storage_tx,
                        subtask_tx: ctx.subtask_tx,
                        command_rx: w_cmd_rx,
                        token: token.clone(),
                    },
                    handshake.extension_protocol,
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
