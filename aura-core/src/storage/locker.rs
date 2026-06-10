use crate::{Result, TaskId};
use std::collections::HashSet;
use tokio::fs::File;

pub struct AdvisoryLocker {
    network_shares: HashSet<TaskId>,
}

impl Default for AdvisoryLocker {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvisoryLocker {
    pub fn new() -> Self {
        Self {
            network_shares: HashSet::new(),
        }
    }

    pub fn lock_and_detect_network(&mut self, id: TaskId, file: &File) -> Result<()> {
        crate::storage::sys::try_lock_file(file)?;

        if crate::storage::sys::is_network_share(file) {
            self.network_shares.insert(id);
        }

        Ok(())
    }

    pub fn is_network_share(&self, id: &TaskId) -> bool {
        self.network_shares.contains(id)
    }

    pub fn unregister_task(&mut self, id: &TaskId) {
        self.network_shares.remove(id);
    }
}
