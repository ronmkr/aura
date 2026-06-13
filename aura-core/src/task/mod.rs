//! task: Core representations of download tasks and their lifecycles.

pub mod extension;
pub mod meta;
pub mod phase;
pub mod range;
pub mod state;
pub mod subtask;

pub use meta::MetaTask;
pub use phase::{DownloadPhase, FollowOnAction, TaskType};
pub use range::Range;
pub use state::TaskState;
pub use subtask::SubTask;
