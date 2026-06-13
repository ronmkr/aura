mod codec;
mod extensions;
mod handshake;
mod messages;

pub use codec::PeerCodec;
pub use extensions::{ExtendedHandshake, MetadataMessage, PexMessage, EXTENSION_BIT};
pub use handshake::{Handshake, HANDSHAKE_LEN, PSTR};
pub use messages::PeerMessage;
pub mod mse;
pub mod mse_handshake_incoming;

#[cfg(test)]
mod mse_edge_tests;
pub mod mse_handshake_outgoing;
#[cfg(test)]
mod mse_tests;

pub const BLOCK_SIZE: u32 = 16384; // 16KB block size (BitTorrent specification standard)
pub type PeerId = [u8; 20];
