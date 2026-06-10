use serde::{Deserialize, Serialize};

/// Represents a byte range [start, end)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct Range {
    pub start: u64,
    pub end: u64,
}

impl Range {
    pub fn length(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }
}
