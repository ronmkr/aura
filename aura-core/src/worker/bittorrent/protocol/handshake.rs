use crate::Error;
use crate::Result;

pub const HANDSHAKE_LEN: usize = 68;
pub const PSTR: &[u8] = b"BitTorrent protocol";

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
