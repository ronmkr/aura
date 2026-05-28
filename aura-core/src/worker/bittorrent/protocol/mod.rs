mod handshake;
mod messages;
mod codec;
mod extensions;

pub use handshake::{Handshake, HANDSHAKE_LEN, PSTR};
pub use messages::PeerMessage;
pub use codec::PeerCodec;
pub use extensions::{ExtendedHandshake, MetadataMessage, PexMessage, EXTENSION_BIT};

pub const BLOCK_SIZE: u32 = 16384; // 16KB block size (BitTorrent specification standard)
pub type PeerId = [u8; 20];
