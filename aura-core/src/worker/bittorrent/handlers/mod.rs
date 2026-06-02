pub(crate) mod data;
pub(crate) mod incoming;

use super::protocol::{PeerCodec, PeerMessage};
use super::BtWorker;
use crate::orchestrator::SubTaskEvent;
use crate::storage::StorageRequest;
use crate::worker::bittorrent::task::BtTask;
use crate::{Result, TaskId};
use tokio_util::codec::Framed;

pub(crate) struct PeerHandlerContext<'a, S> {
    pub(crate) framed: &'a mut Framed<S, PeerCodec>,
    pub(crate) task: &'a BtTask,
    pub(crate) meta_id: TaskId,
    pub(crate) sub_id: TaskId,
    pub(crate) storage_tx: tokio::sync::mpsc::Sender<StorageRequest>,
    pub(crate) subtask_tx: tokio::sync::mpsc::Sender<SubTaskEvent>,
    pub(crate) peer_choking: &'a mut bool,
}

impl BtWorker {
    pub(crate) async fn handle_peer_message<S>(
        &mut self,
        msg: PeerMessage,
        mut ctx: PeerHandlerContext<'_, S>,
    ) -> Result<()>
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
    {
        // First try basic messages (Choke, Unchoke, Have, Bitfield, Extended)
        if self.handle_basic_messages(msg.clone(), &mut ctx).await? {
            return Ok(());
        }

        // Then try data-related messages (Request, Piece, Hashes)
        if self.handle_data_messages(msg, &mut ctx).await? {
            return Ok(());
        }

        Ok(())
    }
}
