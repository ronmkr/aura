use std::net::SocketAddr;

pub type NodeId = [u8; 20];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub id: NodeId,
    pub addr: SocketAddr,
}

#[derive(Debug, Clone, Default)]
pub struct KBucket {
    pub nodes: Vec<Node>,
}

impl KBucket {
    pub fn new() -> Self {
        Self {
            nodes: Vec::with_capacity(8),
        }
    }

    pub fn insert(&mut self, node: Node) {
        if let Some(pos) = self.nodes.iter().position(|n| n.id == node.id) {
            self.nodes.remove(pos);
        }
        if self.nodes.len() < 8 {
            self.nodes.push(node);
        }
    }
}

#[derive(Debug, Clone)]
pub struct RoutingTable {
    pub my_id: NodeId,
    pub buckets: Vec<KBucket>,
}

impl RoutingTable {
    pub fn new(my_id: NodeId) -> Self {
        let mut buckets = Vec::with_capacity(160);
        for _ in 0..160 {
            buckets.push(KBucket::new());
        }
        Self { my_id, buckets }
    }

    pub fn distance(&self, id: &NodeId) -> u128 {
        let mut dist = 0u128;
        for (a, b) in self.my_id.iter().zip(id.iter()) {
            dist = (dist << 8) | (a ^ b) as u128;
        }
        dist
    }

    pub fn bucket_index(&self, id: &NodeId) -> usize {
        for (i, (a, b)) in self.my_id.iter().zip(id.iter()).enumerate() {
            let xor = a ^ b;
            if xor != 0 {
                return i * 8 + xor.leading_zeros() as usize;
            }
        }
        159
    }

    pub fn insert(&mut self, node: Node) {
        if node.id == self.my_id {
            return;
        }
        let idx = self.bucket_index(&node.id);
        self.buckets[idx].insert(node);
    }

    pub fn get_closest_nodes(&self, target: &NodeId, k: usize) -> Vec<Node> {
        let mut all_nodes = Vec::new();
        for bucket in &self.buckets {
            all_nodes.extend(bucket.nodes.clone());
        }

        all_nodes.sort_by_key(|n| {
            let mut dist = 0u128;
            for (a, b) in n.id.iter().zip(target.iter()) {
                dist = (dist << 8) | (a ^ b) as u128;
            }
            dist
        });

        all_nodes.truncate(k);
        all_nodes
    }
}

#[cfg(test)]
#[path = "routing_tests.rs"]
mod tests;
