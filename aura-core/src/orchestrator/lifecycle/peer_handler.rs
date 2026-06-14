use crate::orchestrator::{SubTaskEvent, WorkerCommand};
use crate::worker::bittorrent::protocol::mse::MseStream;
use crate::worker::bittorrent::task::BtTask;
use crate::worker::bittorrent::BtWorker;
use crate::{Error, InfoHash, Result, TaskId};
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
    pub storage_client: Arc<dyn crate::storage::StorageDispatch>,
    pub subtask_tx: mpsc::Sender<SubTaskEvent>,
    pub cancellation_tokens: std::collections::HashMap<TaskId, CancellationToken>,
    pub throttler: Arc<crate::throttler::Throttler>,
    pub(crate) orchestrator_handle: super::super::state::OrchestratorHandle,
}

pub async fn handle_incoming_peer(
    mut stream: TcpStream,
    addr: std::net::SocketAddr,
    ctx: IncomingPeerContext,
) -> Result<()> {
    use crate::worker::bittorrent::Handshake;
    use crate::worker::bittorrent::HANDSHAKE_LEN;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let policy = ctx.orchestrator_handle.config.load().bittorrent.encryption;
    let connect_timeout_secs = ctx
        .orchestrator_handle
        .config
        .load()
        .network
        .connect_timeout_secs;

    let (mut mse_stream, handshake) = tokio::time::timeout(
        std::time::Duration::from_secs(connect_timeout_secs),
        async {
            let mut first_byte = [0u8; 1];
            stream
                .read_exact(&mut first_byte)
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?;

            if first_byte[0] == 0x13 {
                // Plaintext handshake
                if policy == crate::config::EncryptionPolicy::Require {
                    return Err(Error::Protocol(
                        "Encryption required but incoming connection is plaintext".to_string(),
                    ));
                }
                let mut remaining = [0u8; HANDSHAKE_LEN - 1];
                stream
                    .read_exact(&mut remaining)
                    .await
                    .map_err(|e| Error::Protocol(e.to_string()))?;
                let mut buf = [0u8; HANDSHAKE_LEN];
                buf[0] = 0x13;
                buf[1..].copy_from_slice(&remaining);
                let handshake = Handshake::deserialize(&buf)?;
                Ok((MseStream::new(stream), handshake))
            } else {
                // MSE handshake
                if policy == crate::config::EncryptionPolicy::Disable {
                    return Err(Error::Protocol(
                        "Encryption disabled but incoming connection uses MSE".to_string(),
                    ));
                }
                let mut remaining = [0u8; 95];
                stream
                    .read_exact(&mut remaining)
                    .await
                    .map_err(|e| Error::Protocol(e.to_string()))?;
                let mut ya = [0u8; 96];
                ya[0] = first_byte[0];
                ya[1..].copy_from_slice(&remaining);

                let mut mse_stream = MseStream::new(stream);
                let active_torrents: Vec<crate::InfoHash> =
                    ctx.bt_registry.keys().cloned().collect();
                let (_target_info_hash, ia) = mse_stream
                    .handshake_incoming(ya, policy, &active_torrents)
                    .await?;

                let handshake = if ia.len() >= HANDSHAKE_LEN {
                    Handshake::deserialize(&ia[..HANDSHAKE_LEN])?
                } else {
                    let mut buf = [0u8; HANDSHAKE_LEN];
                    buf[..ia.len()].copy_from_slice(&ia);
                    mse_stream
                        .read_exact(&mut buf[ia.len()..])
                        .await
                        .map_err(|e| Error::Protocol(e.to_string()))?;
                    Handshake::deserialize(&buf)?
                };
                Ok((mse_stream, handshake))
            }
        },
    )
    .await
    .map_err(|_| Error::Protocol("Incoming peer handshake timeout".to_string()))??;

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

            let my_handshake = Handshake::new(handshake.info_hash, ctx.orchestrator_handle.peer_id);
            mse_stream
                .write_all(&my_handshake.serialize())
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?;

            let mut worker = BtWorker::new(ctx.orchestrator_handle.build_bt_worker_options(
                addr.to_string(),
                target_info_hash,
                handshake.peer_id,
                ctx.throttler,
            ));
            worker.local_addr = ctx.orchestrator_handle.resolve_local_addr();

            let w_cmd_rx = if let Some(tx) = ctx.worker_command_txs.get(&task.id) {
                tx.subscribe()
            } else {
                let (dummy_tx, _) = tokio::sync::broadcast::channel::<WorkerCommand>(1024);
                dummy_tx.subscribe()
            };

            worker
                .run_loop_with_stream(
                    mse_stream,
                    crate::worker::bittorrent::BtWorkerArgs {
                        meta_id: task.id,
                        sub_id: task.id,
                        task: task.clone(),
                        storage_client: ctx.storage_client,
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
