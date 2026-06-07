pub mod handlers;
pub mod logic;
pub mod merkle;
pub mod protocol;
pub mod task;
#[cfg(test)]
mod tests;
pub mod worker;

pub use protocol::{
    ExtendedHandshake, Handshake, MetadataMessage, PeerCodec, PeerId, PeerMessage, BLOCK_SIZE,
    HANDSHAKE_LEN,
};
pub use worker::*;

pub const BT_EXTENSION_KEY: &str = "bittorrent";
