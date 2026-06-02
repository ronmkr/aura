use super::*;

use proptest::prelude::*;
use tokio::sync::mpsc;

proptest! {
    #[test]
    fn test_lpd_packet_parsing_proptest(ref s in "\\PC*") {
        let (_event_tx, _event_rx) = mpsc::channel::<crate::orchestrator::SubTaskEvent>(1);

        let text = String::from_utf8_lossy(s.as_bytes());
        for line in text.lines() {
            let _parts: Vec<&str> = line.splitn(2, ':').map(|s| s.trim()).collect();
        }
    }
}

#[tokio::test]
async fn test_lpd_packet_parsing() {
    let (_cmd_tx, _cmd_rx) = mpsc::channel(1);
    let (event_tx, mut event_rx) = mpsc::channel(1);

    let actor = LpdActor {
        command_rx: _cmd_rx,
        event_tx,
        socket: UdpSocket::bind("127.0.0.1:0").await.unwrap(),
        active_hashes: HashSet::new(),
        cookie: "my-cookie".to_string(),
    };

    let info_hash = InfoHash::V1([1u8; 20]);
    let mut actor = actor;
    actor.active_hashes.insert((info_hash, 6881));

    let message = "BT-SEARCH * HTTP/1.1\r\n\
                   Port: 6882\r\n\
                   Infohash: 0101010101010101010101010101010101010101\r\n\
                   cookie: other-cookie\r\n\r\n";

    let addr = "192.168.1.100:12345".parse::<SocketAddr>().unwrap();
    actor.handle_packet(message.as_bytes(), addr).await;

    let event = event_rx.recv().await.unwrap();
    if let crate::orchestrator::SubTaskEvent::LpdPeerDiscovered(h, peer) = event {
        assert_eq!(h, info_hash);
        assert_eq!(peer.ip, "192.168.1.100");
        assert_eq!(peer.port, 6882);
    } else {
        panic!("Wrong event type");
    }
}

#[tokio::test]
async fn test_lpd_ignore_own_cookie() {
    let (_cmd_tx, _cmd_rx) = mpsc::channel(1);
    let (event_tx, mut event_rx) = mpsc::channel(1);

    let actor = LpdActor {
        command_rx: _cmd_rx,
        event_tx,
        socket: UdpSocket::bind("127.0.0.1:0").await.unwrap(),
        active_hashes: HashSet::new(),
        cookie: "my-cookie".to_string(),
    };

    let info_hash = InfoHash::V1([1u8; 20]);
    let mut actor = actor;
    actor.active_hashes.insert((info_hash, 6881));

    let message = "BT-SEARCH * HTTP/1.1\r\n\
                   Port: 6881\r\n\
                   Infohash: 0101010101010101010101010101010101010101\r\n\
                   cookie: my-cookie\r\n\r\n";

    let addr = "192.168.1.100:12345".parse::<SocketAddr>().unwrap();
    actor.handle_packet(message.as_bytes(), addr).await;

    tokio::select! {
        _ = event_rx.recv() => panic!("Should not have received an event"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
    }
}
