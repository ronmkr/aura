use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const EXTENSION_BIT: usize = 20; // 20th bit in reserved bytes (counting from end)

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtendedHandshake {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub m: Option<HashMap<String, u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata_size: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MetadataMessage {
    pub msg_type: u8, // 0: request, 1: data, 2: reject
    pub piece: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_size: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default, PartialEq)]
pub struct PexMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added: Option<serde_bytes::ByteBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added_f: Option<serde_bytes::ByteBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dropped: Option<serde_bytes::ByteBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added6: Option<serde_bytes::ByteBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added6_f: Option<serde_bytes::ByteBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dropped6: Option<serde_bytes::ByteBuf>,
}

impl PexMessage {
    pub fn encode_peers(
        added_peers: &[std::net::SocketAddr],
        dropped_peers: &[std::net::SocketAddr],
    ) -> Self {
        // BEP 11 mandates we drop excess peers if they exceed 65 to prevent bloat.
        let added_limited: Vec<_> = added_peers.iter().take(65).copied().collect();
        let dropped_limited: Vec<_> = dropped_peers.iter().take(65).copied().collect();

        let (added, added6, added_f, added6_f) = Self::encode_address_list(&added_limited);
        let (dropped, dropped6, _, _) = Self::encode_address_list(&dropped_limited);

        Self {
            added,
            added_f,
            dropped,
            added6,
            added6_f,
            dropped6,
        }
    }

    fn encode_address_list(
        peers: &[std::net::SocketAddr],
    ) -> (
        Option<serde_bytes::ByteBuf>,
        Option<serde_bytes::ByteBuf>,
        Option<serde_bytes::ByteBuf>,
        Option<serde_bytes::ByteBuf>,
    ) {
        let mut v4 = Vec::new();
        let mut v6 = Vec::new();
        let mut v4_flags = Vec::new();
        let mut v6_flags = Vec::new();

        for p in peers {
            match p.ip() {
                std::net::IpAddr::V4(ip) => {
                    v4.extend_from_slice(&ip.octets());
                    v4.extend_from_slice(&p.port().to_be_bytes());
                    v4_flags.push(0u8); // Default flags: 0x00
                }
                std::net::IpAddr::V6(ip) => {
                    v6.extend_from_slice(&ip.octets());
                    v6.extend_from_slice(&p.port().to_be_bytes());
                    v6_flags.push(0u8);
                }
            }
        }
        (
            if v4.is_empty() {
                None
            } else {
                Some(serde_bytes::ByteBuf::from(v4))
            },
            if v6.is_empty() {
                None
            } else {
                Some(serde_bytes::ByteBuf::from(v6))
            },
            if v4_flags.is_empty() {
                None
            } else {
                Some(serde_bytes::ByteBuf::from(v4_flags))
            },
            if v6_flags.is_empty() {
                None
            } else {
                Some(serde_bytes::ByteBuf::from(v6_flags))
            },
        )
    }

    pub fn decode_peers(&self) -> Vec<crate::tracker::Peer> {
        let mut peers = Vec::new();
        if let Some(ref b) = self.added {
            for chunk in b.chunks_exact(6) {
                let ip = std::net::Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
                let port = u16::from_be_bytes([chunk[4], chunk[5]]);
                peers.push(crate::tracker::Peer {
                    id: None,
                    ip: ip.to_string(),
                    port,
                });
            }
        }
        if let Some(ref b) = self.added6 {
            for chunk in b.chunks_exact(18) {
                let mut ip_bytes = [0u8; 16];
                ip_bytes.copy_from_slice(&chunk[0..16]);
                let ip = std::net::Ipv6Addr::from(ip_bytes);
                let port = u16::from_be_bytes([chunk[16], chunk[17]]);
                peers.push(crate::tracker::Peer {
                    id: None,
                    ip: ip.to_string(),
                    port,
                });
            }
        }
        peers
    }
}

#[cfg(test)]
#[path = "extensions_tests.rs"]
mod tests;
