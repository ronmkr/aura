use super::*;

#[test]
fn test_parse_compact_peers() {
    let client = TrackerClient::new([0; 20], 6881, None, None, None);
    let bytes = vec![127, 0, 0, 1, 0x1a, 0xe1]; // 127.0.0.1:6881
    let peers = client.parse_compact_peers_raw(&bytes).unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].ip, "127.0.0.1");
    assert_eq!(peers[0].port, 6881);
}

#[test]
fn test_parse_non_compact_peers() {
    let client = TrackerClient::new([0; 20], 6881, None, None, None);

    use std::collections::HashMap;
    let mut peer_dict = HashMap::new();
    peer_dict.insert(
        b"ip".to_vec(),
        serde_bencode::value::Value::Bytes(b"127.0.0.1".to_vec()),
    );
    peer_dict.insert(b"port".to_vec(), serde_bencode::value::Value::Int(6881));

    let peers_val =
        serde_bencode::value::Value::List(vec![serde_bencode::value::Value::Dict(peer_dict)]);

    let peers = client.parse_peers(peers_val).unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].ip, "127.0.0.1");
    assert_eq!(peers[0].port, 6881);
}
