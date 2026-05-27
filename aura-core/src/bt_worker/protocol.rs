use crate::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder};

pub const HANDSHAKE_LEN: usize = 68;
pub const PSTR: &[u8] = b"BitTorrent protocol";
pub const BLOCK_SIZE: u32 = 16384; // 16KB block size (BitTorrent specification standard)

pub const EXTENSION_BIT: usize = 20; // 20th bit in reserved bytes (counting from end)

pub type PeerId = [u8; 20];

/// Represents a BitTorrent handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handshake {
    pub info_hash: [u8; 20],
    pub peer_id: [u8; 20],
    pub extension_protocol: bool,
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Self {
            info_hash,
            peer_id,
            extension_protocol: true,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HANDSHAKE_LEN);
        buf.push(PSTR.len() as u8);
        buf.extend_from_slice(PSTR);

        let mut reserved = [0u8; 8];
        if self.extension_protocol {
            reserved[5] |= 0x10; // 20th bit (BEP 10)
        }
        buf.extend_from_slice(&reserved);

        buf.extend_from_slice(&self.info_hash);
        buf.extend_from_slice(&self.peer_id);
        buf
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        if data.len() < HANDSHAKE_LEN {
            return Err(Error::Protocol("Handshake too short".to_string()));
        }
        let pstr_len = data[0] as usize;
        if pstr_len != PSTR.len() || &data[1..1 + pstr_len] != PSTR {
            return Err(Error::Protocol("Invalid protocol string".to_string()));
        }

        let reserved = &data[20..28];
        let extension_protocol = (reserved[5] & 0x10) != 0;

        let mut info_hash = [0; 20];
        info_hash.copy_from_slice(&data[28..48]);
        let mut peer_id = [0; 20];
        peer_id.copy_from_slice(&data[48..68]);
        Ok(Self {
            info_hash,
            peer_id,
            extension_protocol,
        })
    }
}

/// BitTorrent peer messages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeerMessage {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have(u32),
    Bitfield(Vec<u8>),
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        block: Bytes,
    },
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
    Extended {
        id: u8,
        payload: Bytes,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtendedHandshake {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub m: Option<std::collections::HashMap<String, u8>>,
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

impl PeerMessage {
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            PeerMessage::KeepAlive => {
                buf.put_u32(0);
            }
            PeerMessage::Choke => {
                buf.put_u32(1);
                buf.put_u8(0);
            }
            PeerMessage::Unchoke => {
                buf.put_u32(1);
                buf.put_u8(1);
            }
            PeerMessage::Interested => {
                buf.put_u32(1);
                buf.put_u8(2);
            }
            PeerMessage::NotInterested => {
                buf.put_u32(1);
                buf.put_u8(3);
            }
            PeerMessage::Have(idx) => {
                buf.put_u32(5);
                buf.put_u8(4);
                buf.put_u32(*idx);
            }
            PeerMessage::Bitfield(bits) => {
                buf.put_u32(1 + bits.len() as u32);
                buf.put_u8(5);
                buf.extend_from_slice(bits);
            }
            PeerMessage::Request {
                index,
                begin,
                length,
            } => {
                buf.put_u32(13);
                buf.put_u8(6);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
            }
            PeerMessage::Piece {
                index,
                begin,
                block,
            } => {
                buf.put_u32(9 + block.len() as u32);
                buf.put_u8(7);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.extend_from_slice(block);
            }
            PeerMessage::Cancel {
                index,
                begin,
                length,
            } => {
                buf.put_u32(13);
                buf.put_u8(8);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
            }
            PeerMessage::Extended { id, payload } => {
                buf.put_u32(2 + payload.len() as u32);
                buf.put_u8(20);
                buf.put_u8(*id);
                buf.extend_from_slice(payload);
            }
        }
        buf
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let id = data[0];
        let mut data_ref = &data[1..];
        match id {
            0 => Ok(PeerMessage::Choke),
            1 => Ok(PeerMessage::Unchoke),
            2 => Ok(PeerMessage::Interested),
            3 => Ok(PeerMessage::NotInterested),
            4 => Ok(PeerMessage::Have(data_ref.get_u32())),
            5 => Ok(PeerMessage::Bitfield(data_ref.to_vec())),
            6 => Ok(PeerMessage::Request {
                index: data_ref.get_u32(),
                begin: data_ref.get_u32(),
                length: data_ref.get_u32(),
            }),
            7 => {
                let index = data_ref.get_u32();
                let begin = data_ref.get_u32();
                Ok(PeerMessage::Piece {
                    index,
                    begin,
                    block: Bytes::copy_from_slice(data_ref),
                })
            }
            8 => Ok(PeerMessage::Cancel {
                index: data_ref.get_u32(),
                begin: data_ref.get_u32(),
                length: data_ref.get_u32(),
            }),
            20 => {
                let extended_id = data_ref.get_u8();
                Ok(PeerMessage::Extended {
                    id: extended_id,
                    payload: Bytes::copy_from_slice(data_ref),
                })
            }
            _ => Err(Error::Protocol(format!("Unknown message ID: {}", id))),
        }
    }
}

pub struct PeerCodec;

impl Decoder for PeerCodec {
    type Item = PeerMessage;
    type Error = Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>> {
        if src.len() < 4 {
            return Ok(None);
        }

        let length = u32::from_be_bytes([src[0], src[1], src[2], src[3]]) as usize;

        if length == 0 {
            src.advance(4);
            return Ok(Some(PeerMessage::KeepAlive));
        }

        if src.len() < 4 + length {
            src.reserve(4 + length - src.len());
            return Ok(None);
        }

        src.advance(4);
        let data = src.split_to(length);
        let msg = PeerMessage::deserialize(&data)?;
        Ok(Some(msg))
    }
}

impl Encoder<PeerMessage> for PeerCodec {
    type Error = Error;

    fn encode(&mut self, item: PeerMessage, dst: &mut BytesMut) -> Result<()> {
        let serialized = item.serialize();
        dst.extend_from_slice(&serialized);
        Ok(())
    }
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
        let (added, added6) = Self::encode_address_list(added_peers);
        let (dropped, dropped6) = Self::encode_address_list(dropped_peers);

        Self {
            added,
            added_f: None, // flags not strictly required for MVP, but can be added later
            dropped,
            added6,
            added6_f: None,
            dropped6,
        }
    }

    fn encode_address_list(
        peers: &[std::net::SocketAddr],
    ) -> (Option<serde_bytes::ByteBuf>, Option<serde_bytes::ByteBuf>) {
        let mut v4 = Vec::new();
        let mut v6 = Vec::new();
        for p in peers {
            match p.ip() {
                std::net::IpAddr::V4(ip) => {
                    v4.extend_from_slice(&ip.octets());
                    v4.extend_from_slice(&p.port().to_be_bytes());
                }
                std::net::IpAddr::V6(ip) => {
                    v6.extend_from_slice(&ip.octets());
                    v6.extend_from_slice(&p.port().to_be_bytes());
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
mod tests {
    use super::*;

    #[test]
    fn test_pex_message_encoding() {
        let p1 = "192.168.1.1:8080".parse().unwrap();
        let p2 = "[2001:db8::1]:9090".parse().unwrap();
        let msg = PexMessage::encode_peers(&[p1, p2], &[]);

        let bencoded = serde_bencode::to_bytes(&msg).unwrap();
        let decoded: PexMessage = serde_bencode::from_bytes(&bencoded).unwrap();

        let peers = decoded.decode_peers();
        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].ip, "192.168.1.1");
        assert_eq!(peers[0].port, 8080);
        assert_eq!(peers[1].ip, "2001:db8::1");
        assert_eq!(peers[1].port, 9090);
    }
}
