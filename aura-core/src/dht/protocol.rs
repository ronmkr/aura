use super::routing::Node;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KrpcMessage {
    #[serde(rename = "t")]
    pub transaction_id: Vec<u8>,
    #[serde(rename = "y")]
    pub msg_type: String, // "q", "r", "e"
    #[serde(rename = "q", skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(rename = "a", skip_serializing_if = "Option::is_none")]
    pub args: Option<BTreeMap<String, serde_bencode::value::Value>>,
    #[serde(rename = "r", skip_serializing_if = "Option::is_none")]
    pub response: Option<BTreeMap<String, serde_bencode::value::Value>>,
    #[serde(rename = "e", skip_serializing_if = "Option::is_none")]
    pub error: Option<(u32, String)>,
}

impl KrpcMessage {
    pub fn encode(&self) -> Result<Vec<u8>> {
        serde_bencode::to_bytes(self)
            .map_err(|e| Error::Protocol(format!("Failed to encode KRPC: {}", e)))
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        serde_bencode::from_bytes(data)
            .map_err(|e| Error::Protocol(format!("Failed to decode KRPC: {}", e)))
    }
}

pub fn compact_nodes(nodes: &[Node]) -> Vec<u8> {
    let mut compact = Vec::with_capacity(nodes.len() * 26);
    for node in nodes {
        compact.extend_from_slice(&node.id);
        match node.addr.ip() {
            IpAddr::V4(v4) => {
                compact.extend_from_slice(&v4.octets());
            }
            IpAddr::V6(v6) => {
                compact.extend_from_slice(&v6.octets());
            }
        }
        compact.extend_from_slice(&node.addr.port().to_be_bytes());
    }
    compact
}

pub fn parse_compact_nodes(data: &[u8]) -> Vec<Node> {
    let mut nodes = Vec::new();
    let chunk_size = 26; // 20 (id) + 4 (ip) + 2 (port) for IPv4. DHT spec mostly assumes IPv4 compact.

    for chunk in data.chunks_exact(chunk_size) {
        let mut id = [0u8; 20];
        id.copy_from_slice(&chunk[..20]);
        let ip = Ipv4Addr::new(chunk[20], chunk[21], chunk[22], chunk[23]);
        let port = u16::from_be_bytes([chunk[24], chunk[25]]);
        nodes.push(Node {
            id,
            addr: SocketAddr::new(IpAddr::V4(ip), port),
        });
    }
    nodes
}

pub fn compact_peer(addr: &SocketAddr) -> Vec<u8> {
    let mut compact = Vec::with_capacity(6);
    if let SocketAddr::V4(v4) = addr {
        compact.extend_from_slice(&v4.ip().octets());
        compact.extend_from_slice(&v4.port().to_be_bytes());
    }
    compact
}

pub fn parse_compact_peer(data: &[u8]) -> Option<SocketAddr> {
    if data.len() < 6 {
        return None;
    }
    let ip = Ipv4Addr::new(data[0], data[1], data[2], data[3]);
    let port = u16::from_be_bytes([data[4], data[5]]);
    Some(SocketAddr::new(IpAddr::V4(ip), port))
}

#[cfg(test)]
#[path = "protocol_tests.rs"]
mod tests;
