use super::DhtActor;
use std::net::SocketAddr;
use tracing::{debug, info};

#[derive(serde::Serialize, serde::Deserialize)]
struct PersistedNode {
    id: [u8; 20],
    addr: SocketAddr,
}

#[async_trait::async_trait]
impl crate::dht::PersistentState for DhtActor {
    async fn save(&self, path: &std::path::Path) -> crate::Result<()> {
        let rt = self.routing_table.lock().await;
        let mut nodes = Vec::new();
        for bucket in &rt.buckets {
            for node in &bucket.nodes {
                nodes.push(PersistedNode {
                    id: node.id,
                    addr: node.addr,
                });
            }
        }
        drop(rt);

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let json = serde_json::to_string_pretty(&nodes)
            .map_err(|e| crate::Error::Storage(format!("Failed to serialize DHT nodes: {}", e)))?;

        tokio::fs::write(path, json).await?;
        debug!(
            ?path,
            count = nodes.len(),
            "Saved DHT routing table to file"
        );
        Ok(())
    }

    async fn load(&mut self, path: &std::path::Path) -> crate::Result<()> {
        if !path.exists() {
            return Ok(());
        }

        let data = tokio::fs::read(path).await?;
        let nodes: Vec<PersistedNode> = serde_json::from_slice(&data).map_err(|e| {
            crate::Error::Storage(format!("Failed to deserialize DHT nodes: {}", e))
        })?;

        let mut rt = self.routing_table.lock().await;
        let count = nodes.len();
        for p_node in nodes {
            rt.insert(crate::dht::routing::Node {
                id: p_node.id,
                addr: p_node.addr,
            });
            // Try to ping the loaded node to verify/refresh it
            let _ = self.send_ping(p_node.addr).await;
        }
        info!(?path, count, "Loaded DHT routing table from file");
        Ok(())
    }
}
