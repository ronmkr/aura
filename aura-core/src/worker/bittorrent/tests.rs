#[cfg(test)]
use crate::worker::bittorrent::protocol::{Handshake, PeerCodec, PeerMessage};
#[cfg(test)]
use bytes::{Bytes, BytesMut};
#[cfg(test)]
use tokio_util::codec::{Decoder, Encoder};

#[test]
fn test_handshake_serialization() {
    let info_hash = [1u8; 20];
    let peer_id = [2u8; 20];
    let handshake = Handshake::new(info_hash, peer_id);
    let serialized = handshake.serialize();
    let deserialized = Handshake::deserialize(&serialized).unwrap();
    assert_eq!(handshake.info_hash, deserialized.info_hash);
    assert_eq!(handshake.peer_id, deserialized.peer_id);
    assert!(deserialized.extension_protocol);
}

#[test]
fn test_message_serialization() {
    let msg = PeerMessage::Have(123);
    let serialized = msg.serialize();
    let deserialized = PeerMessage::deserialize(&serialized[4..]).unwrap(); // Skip length prefix
    assert_eq!(msg, deserialized);
}

#[test]
fn test_piece_message_serialization() {
    let block = Bytes::from(vec![1, 2, 3, 4]);
    let msg = PeerMessage::Piece {
        index: 1,
        begin: 0,
        block: block.clone(),
    };
    let mut buf = BytesMut::new();
    let mut codec = PeerCodec;
    codec.encode(msg.clone(), &mut buf).unwrap();
    let decoded = codec.decode(&mut buf).unwrap().unwrap();
    assert_eq!(msg, decoded);
}

#[test]
fn test_hash_request_serialization() {
    let msg = PeerMessage::HashRequest {
        pieces_root: [1u8; 32],
        index: 0,
        base: 0,
        length: 10,
        proof_layers: 0,
    };
    let serialized = msg.serialize();
    let deserialized = PeerMessage::deserialize(&serialized[4..]).unwrap();
    assert_eq!(msg, deserialized);
}

#[test]
fn test_hashes_message_serialization() {
    let msg = PeerMessage::Hashes {
        pieces_root: [1u8; 32],
        index: 0,
        base: 0,
        length: 2,
        proof_layers: 0,
        hashes: vec![[2u8; 32], [3u8; 32]],
    };
    let serialized = msg.serialize();
    let deserialized = PeerMessage::deserialize(&serialized[4..]).unwrap();
    assert_eq!(msg, deserialized);
}
