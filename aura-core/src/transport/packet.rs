//! uTP Packet serialization and deserialization (BEP 29).

use crate::{Error, Result};

/// uTP Packet Types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketType {
    Data = 0,
    Fin = 1,
    State = 2,
    Reset = 3,
    Syn = 4,
}

impl PacketType {
    pub fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Self::Data),
            1 => Some(Self::Fin),
            2 => Some(Self::State),
            3 => Some(Self::Reset),
            4 => Some(Self::Syn),
            _ => None,
        }
    }
}

/// uTP Packet Header (20 bytes).
#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub packet_type: PacketType,
    pub version: u8,
    pub extension: u8,
    pub connection_id: u16,
    pub timestamp_us: u32,
    pub timestamp_difference_us: u32,
    pub wnd_size: u32,
    pub seq_nr: u16,
    pub ack_nr: u16,
}

impl PacketHeader {
    pub const LEN: usize = 20;

    /// Serializes the header into a byte buffer.
    pub fn serialize(&self, buf: &mut [u8]) {
        assert!(buf.len() >= Self::LEN);
        let type_ver = ((self.packet_type as u8) << 4) | (self.version & 0x0F);
        buf[0] = type_ver;
        buf[1] = self.extension;
        buf[2..4].copy_from_slice(&self.connection_id.to_be_bytes());
        buf[4..8].copy_from_slice(&self.timestamp_us.to_be_bytes());
        buf[8..12].copy_from_slice(&self.timestamp_difference_us.to_be_bytes());
        buf[12..16].copy_from_slice(&self.wnd_size.to_be_bytes());
        buf[16..18].copy_from_slice(&self.seq_nr.to_be_bytes());
        buf[18..20].copy_from_slice(&self.ack_nr.to_be_bytes());
    }

    /// Deserializes a header from a byte slice.
    pub fn deserialize(buf: &[u8]) -> Result<Self> {
        if buf.len() < Self::LEN {
            return Err(Error::Protocol(
                "Buffer too small for uTP header".to_string(),
            ));
        }
        let type_ver = buf[0];
        let packet_type_val = type_ver >> 4;
        let version = type_ver & 0x0F;

        let packet_type = PacketType::from_u8(packet_type_val).ok_ok_or_else(|| {
            Error::Protocol(format!("Invalid uTP packet type: {}", packet_type_val))
        })?;

        if version != 1 {
            return Err(Error::Protocol(format!(
                "Unsupported uTP version: {}",
                version
            )));
        }

        let extension = buf[1];

        let mut conn_id_bytes = [0u8; 2];
        conn_id_bytes.copy_from_slice(&buf[2..4]);
        let connection_id = u16::from_be_bytes(conn_id_bytes);

        let mut ts_bytes = [0u8; 4];
        ts_bytes.copy_from_slice(&buf[4..8]);
        let timestamp_us = u32::from_be_bytes(ts_bytes);

        let mut diff_bytes = [0u8; 4];
        diff_bytes.copy_from_slice(&buf[8..12]);
        let timestamp_difference_us = u32::from_be_bytes(diff_bytes);

        let mut wnd_bytes = [0u8; 4];
        wnd_bytes.copy_from_slice(&buf[12..16]);
        let wnd_size = u32::from_be_bytes(wnd_bytes);

        let mut seq_bytes = [0u8; 2];
        seq_bytes.copy_from_slice(&buf[16..18]);
        let seq_nr = u16::from_be_bytes(seq_bytes);

        let mut ack_bytes = [0u8; 2];
        ack_bytes.copy_from_slice(&buf[18..20]);
        let ack_nr = u16::from_be_bytes(ack_bytes);

        Ok(Self {
            packet_type,
            version,
            extension,
            connection_id,
            timestamp_us,
            timestamp_difference_us,
            wnd_size,
            seq_nr,
            ack_nr,
        })
    }
}

/// Helper extension trait to map Option to Result since we want to avoid unwrap.
trait OptionExt<T> {
    fn ok_ok_or_else<F: FnOnce() -> Error>(self, f: F) -> Result<T>;
}

impl<T> OptionExt<T> for Option<T> {
    fn ok_ok_or_else<F: FnOnce() -> Error>(self, f: F) -> Result<T> {
        match self {
            Some(v) => Ok(v),
            None => Err(f()),
        }
    }
}
