use super::{Peer, TrackerClient};
use crate::{Error, Result};
use std::net::Ipv4Addr;

impl TrackerClient {
    pub(crate) fn parse_peers(&self, peers_val: serde_bencode::value::Value) -> Result<Vec<Peer>> {
        match peers_val {
            serde_bencode::value::Value::List(list) => {
                let mut peers = Vec::new();
                for p in list {
                    if let serde_bencode::value::Value::Dict(dict) = p {
                        let ip = if let Some(serde_bencode::value::Value::Bytes(b)) =
                            dict.get(b"ip".as_slice())
                        {
                            String::from_utf8_lossy(b).to_string()
                        } else {
                            continue;
                        };
                        let port = if let Some(serde_bencode::value::Value::Int(p)) =
                            dict.get(b"port".as_slice())
                        {
                            *p as u16
                        } else {
                            continue;
                        };
                        peers.push(Peer {
                            id: dict.get(b"peer id".as_slice()).cloned(),
                            ip,
                            port,
                        });
                    }
                }
                Ok(peers)
            }
            serde_bencode::value::Value::Bytes(bytes) => self.parse_compact_peers_raw(&bytes),
            _ => Err(Error::Protocol("Invalid peers format".to_string())),
        }
    }

    pub(crate) fn parse_compact_peers_raw(&self, bytes: &[u8]) -> Result<Vec<Peer>> {
        let mut peers = Vec::new();
        for chunk in bytes.chunks_exact(6) {
            let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
            let port = u16::from_be_bytes([chunk[4], chunk[5]]);
            peers.push(Peer {
                id: None,
                ip: ip.to_string(),
                port,
            });
        }
        Ok(peers)
    }
}
