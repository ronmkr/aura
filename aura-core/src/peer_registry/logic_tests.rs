use super::*;

#[test]
fn test_peer_registry_basics() {
    let mut registry = PeerRegistry::new();
    let p1 = Peer {
        id: None,
        ip: "1.1.1.1".to_string(),
        port: 80,
    };
    let p2 = Peer {
        id: None,
        ip: "2.2.2.2".to_string(),
        port: 80,
    };

    registry.add_peers(vec![p1.clone(), p2.clone()]);
    assert_eq!(registry.peer_count(), 2);

    let to_connect = registry.get_peer_to_connect().unwrap();
    assert!(to_connect.ip == "1.1.1.1" || to_connect.ip == "2.2.2.2");

    let to_connect2 = registry.get_peer_to_connect().unwrap();
    assert!(to_connect2.ip == "1.1.1.1" || to_connect2.ip == "2.2.2.2");
    assert_ne!(to_connect.ip, to_connect2.ip);
}

#[test]
fn test_peer_registry_rates() {
    let mut registry = PeerRegistry::new();
    let p1 = Peer {
        id: None,
        ip: "1.1.1.1".to_string(),
        port: 80,
    };
    registry.add_peers(vec![p1]);

    let addr = "1.1.1.1:80";
    registry.update_state(addr, ConnectionState::Handshaked);
    registry.add_downloaded(addr, 1024);

    registry.tick_rates(1.0);
    let connected = registry.get_all_connected();
    assert_eq!(connected.len(), 1);
    assert_eq!(connected[0].download_rate, 1024.0);

    // Tick again with no new downloads
    registry.tick_rates(1.0);
    let connected2 = registry.get_all_connected();
    assert_eq!(connected2[0].download_rate, 0.0);
}

#[test]
fn test_peer_reconnect_after_failure() {
    let mut registry = PeerRegistry::new();
    let p1 = Peer {
        id: None,
        ip: "1.1.1.1".to_string(),
        port: 80,
    };
    registry.add_peers(vec![p1]);

    // get_peer_to_connect transitions Disconnected -> Connecting
    let peer = registry.get_peer_to_connect().unwrap();
    assert_eq!(peer.ip, "1.1.1.1");

    // Peer stuck in Connecting: no peers available
    assert!(registry.get_peer_to_connect().is_none());

    // Simulate connection failure: transition back to Disconnected
    registry.update_state("1.1.1.1:80", ConnectionState::Disconnected);

    // Peer slot is freed and available for reconnection
    let peer2 = registry.get_peer_to_connect().unwrap();
    assert_eq!(peer2.ip, "1.1.1.1");
}

#[test]
fn test_peer_stuck_in_connecting_blocks_reconnection() {
    let mut registry = PeerRegistry::new();
    let p1 = Peer {
        id: None,
        ip: "1.1.1.1".to_string(),
        port: 80,
    };
    registry.add_peers(vec![p1]);

    // Transition to Connecting
    let _ = registry.get_peer_to_connect().unwrap();

    // Without update_state back to Disconnected, peer remains stuck
    assert!(registry.get_peer_to_connect().is_none());

    // Re-adding the same peer does not help (entry already exists)
    let p1_again = Peer {
        id: None,
        ip: "1.1.1.1".to_string(),
        port: 80,
    };
    registry.add_peers(vec![p1_again]);
    assert!(registry.get_peer_to_connect().is_none());
}
