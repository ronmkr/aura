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

#[tokio::test]
async fn test_failed_connection_transitions_to_disconnected() {
    use super::{BtWorker, BtWorkerArgs, BtWorkerOptions};
    use crate::peer_registry::ConnectionState;
    use crate::worker::bittorrent::task::BtTask;
    use std::sync::Arc;
    use tokio::sync::{broadcast, mpsc};
    use tokio_util::sync::CancellationToken;

    let temp_dir = tempfile::tempdir().unwrap();
    let db = sled::open(temp_dir.path()).unwrap();
    let (dht_tx, _) = mpsc::channel(1);
    let (lpd_tx, _) = mpsc::channel(1);

    let info_hash = crate::InfoHash::V1([0; 20]);
    let governor = Arc::new(crate::orchestrator::resource_governor::ResourceGovernor::new(0, 0));
    let task = Arc::new(BtTask::from_magnet(
        crate::TaskId(12345),
        info_hash,
        dht_tx,
        lpd_tx,
        db,
        governor,
        None,
        Arc::new(arc_swap::ArcSwap::new(Arc::new(crate::Config::default()))),
    ));

    let peer_addr = "127.0.0.1:45454".to_string();
    {
        let mut registry = task.state.registry.lock().await;
        registry.add_peers(vec![crate::tracker::Peer {
            id: None,
            ip: "127.0.0.1".to_string(),
            port: 45454,
        }]);
        // Transition to Connecting to simulate we are attempting connection
        registry.update_state(&peer_addr, ConnectionState::Connecting);
    }

    let mut worker = BtWorker::new(BtWorkerOptions {
        peer_addr: peer_addr.clone(),
        info_hash,
        peer_id: [0; 20],
        my_id: [0; 20],
        proxy: None,
        throttler: Arc::new(crate::throttler::Throttler::new(0, 0, 100)),
        pex_enabled: false,
        pipeline_size: 10,
        connect_timeout_secs: 5,
        happy_eyeballs_stagger_ms: 250,
    });

    let (storage_tx, _) = mpsc::channel(1);
    let (subtask_tx, _) = mpsc::channel(1);
    let (cmd_tx, _) = broadcast::channel(1);
    let token = CancellationToken::new();

    let args = BtWorkerArgs {
        meta_id: crate::TaskId(12345),
        sub_id: crate::TaskId(12345),
        task: task.clone(),
        storage_tx,
        subtask_tx,
        command_rx: cmd_tx.subscribe(),
        token,
    };

    let res = worker.run_loop(args).await;
    assert!(res.is_err());

    let mut registry = task.state.registry.lock().await;
    let peer_state = registry.get_mut(&peer_addr).unwrap();
    assert_eq!(peer_state.state, ConnectionState::Disconnected);
}
