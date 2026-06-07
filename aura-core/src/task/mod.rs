//! task: Core representations of download tasks and their lifecycles.

pub mod extension;
pub mod meta;
pub mod phase;
pub mod range;
pub mod subtask;

pub use meta::{MetaTask, TaskState};
pub use phase::{DownloadPhase, FollowOnAction, TaskType};
pub use range::Range;
pub use subtask::SubTask;
