pub mod actor;
pub mod protocol;
pub mod routing;

#[cfg(test)]
mod tests;

pub use actor::{DhtActor, DhtCommand};

use std::path::Path;

#[async_trait::async_trait]
pub trait PersistentState {
    async fn save(&self, path: &Path) -> crate::Result<()>;
    async fn load(&mut self, path: &Path) -> crate::Result<()>;
}
