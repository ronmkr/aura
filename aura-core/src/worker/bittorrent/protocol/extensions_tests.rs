use super::*;

#[test]
fn test_pex_message_encoding() {
    let p1 = "192.168.1.1:8080".parse().unwrap();
    let p2 = "[2001:db8::1]:9090".parse().unwrap();
    let msg = PexMessage::encode_peers(&[p1, p2], &[]);

    let bencoded = serde_bencode::to_bytes(&msg).unwrap();
    let decoded: PexMessage = serde_bencode::from_bytes(&bencoded).unwrap();

    let peers = decoded.decode_peers();
    assert_eq!(peers.len(), 2);
    assert_eq!(peers[0].ip, "192.168.1.1");
    assert_eq!(peers[0].port, 8080);
    assert_eq!(peers[1].ip, "2001:db8::1");
    assert_eq!(peers[1].port, 9090);

    // Verify flags were added
    assert!(decoded.added_f.is_some());
    assert_eq!(decoded.added_f.unwrap().len(), 1); // 1 IPv4 peer
    assert!(decoded.added6_f.is_some());
    assert_eq!(decoded.added6_f.unwrap().len(), 1); // 1 IPv6 peer
}

#[test]
fn test_pex_message_encoding_limit() {
    let mut many_peers = Vec::new();
    for i in 0..100 {
        many_peers.push(format!("192.168.1.{}:8080", i % 254 + 1).parse().unwrap());
    }
    let msg = PexMessage::encode_peers(&many_peers, &many_peers);

    let decoded = msg.decode_peers();
    assert_eq!(decoded.len(), 65); // Should be truncated to 65

    assert!(msg.added_f.is_some());
    assert_eq!(msg.added_f.unwrap().len(), 65);
}
