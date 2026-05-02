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
mod tests {
    use super::*;

    #[test]
    fn test_node_id_distance() {
        let id1 = [0u8; 20];
        let mut id2 = [0u8; 20];
        id2[19] = 1;

        let rt = RoutingTable::new(id1);
        assert_eq!(rt.distance(&id2), 1);

        let mut id3 = [0u8; 20];
        id3[18] = 1;
        assert!(rt.distance(&id3) > rt.distance(&id2));
    }

    #[test]
    fn test_bucket_index() {
        let id1 = [0u8; 20];
        let mut id2 = [0u8; 20];
        id2[0] = 0x80; // High bit set

        let rt = RoutingTable::new(id1);
        assert_eq!(rt.bucket_index(&id2), 0);

        let mut id3 = [0u8; 20];
        id3[19] = 1;
        assert_eq!(rt.bucket_index(&id3), 159);
    }

    #[test]
    fn test_get_closest_nodes() {
        let my_id = [0u8; 20];
        let mut rt = RoutingTable::new(my_id);

        for i in 1..10 {
            let mut id = [0u8; 20];
            id[19] = i as u8;
            rt.insert(Node {
                id,
                addr: "127.0.0.1:80".parse().unwrap(),
            });
        }

        let target = [0u8; 20];
        let closest = rt.get_closest_nodes(&target, 5);
        assert_eq!(closest.len(), 5);
    }
}
