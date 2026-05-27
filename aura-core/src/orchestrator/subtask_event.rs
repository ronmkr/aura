use crate::bitfield::Bitfield;
use crate::bt_task::BtTask;
use crate::bt_worker::PeerId;
use crate::task::Range;
use crate::worker::Metadata;
use crate::{InfoHash, TaskId};
use std::sync::Arc;

#[derive(Debug)]
pub enum SubTaskEvent {
    Matured(TaskId, TaskId, Metadata),
    MetadataReceived(TaskId, TaskId, Box<crate::torrent::Torrent>),
    RangeFinished(TaskId, TaskId, Range),
    Failed(TaskId, TaskId, String),
    Downloaded(TaskId, TaskId, u64, String),
    Uploaded(TaskId, TaskId, u64, String),
    PeerBitfield(TaskId, PeerId, Bitfield),
    PeerHave(TaskId, PeerId, u32),
    PieceVerified(TaskId, TaskId, usize),
    BtTaskRegistered(
        TaskId,
        InfoHash,
        Arc<BtTask>,
        tokio::sync::broadcast::Sender<crate::orchestrator::WorkerCommand>,
    ),
    LpdPeerDiscovered(InfoHash, crate::tracker::Peer),
    PexPeersDiscovered(InfoHash, Vec<crate::tracker::Peer>),
    KillSwitch,
    Retry(TaskId, TaskId),
    ScrubberEvent(crate::scrubber::ScrubberEvent),
}
