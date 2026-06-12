pub mod actor;
pub mod protocol;
pub mod routing;

#[cfg(test)]
mod tests;

pub use actor::{DhtActor, DhtCommand};

use std::path::Path;
use tokio::sync::mpsc;
use tracing::warn;

#[async_trait::async_trait]
pub trait PersistentState {
    async fn save(&self, path: &Path) -> crate::Result<()>;
    async fn load(&mut self, path: &Path) -> crate::Result<()>;
}

/// A facade that encapsulates the DHT Kademlia actor and its communication channels.
///
/// `DhtManager` simplifies the instantiation and management of the DHT network,
/// hiding the internal actor channels and routing complexities from the orchestrator.
pub struct DhtManager {
    pub tx: mpsc::Sender<DhtCommand>,
}

impl DhtManager {
    pub async fn new(
        dht_bind_addr: &str,
        dht_id: [u8; 20],
        local_addr: Option<std::net::IpAddr>,
        port: u16,
        db: Option<sled::Db>,
        config: std::sync::Arc<arc_swap::ArcSwap<crate::AuraConfig>>,
        channel_capacity: usize,
    ) -> crate::Result<Self> {
        let (tx, rx) = mpsc::channel(channel_capacity);

        let actor = DhtActor::new(dht_bind_addr, dht_id, rx, local_addr, port, db, config).await?;

        tokio::spawn(async move {
            if let Err(e) = actor.run().await {
                warn!("DHT Actor stopped: {}", e);
            }
        });

        Ok(Self { tx })
    }
}
