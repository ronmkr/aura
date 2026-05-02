use crate::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use serde::{Deserialize, Serialize};
use tokio_util::codec::{Decoder, Encoder};

pub const HANDSHAKE_LEN: usize = 68;
pub const PSTR: &[u8] = b"BitTorrent protocol";
pub const BLOCK_SIZE: u32 = 16384; // 16KB standard block size

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
