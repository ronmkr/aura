use crate::{Error, Result};
use bytes::{Buf, BufMut, Bytes};

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
