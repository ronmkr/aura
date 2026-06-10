use super::*;

#[test]
fn test_parse_compact_peers() {
    let client = TrackerClient::new([0; 20], 6881, None, None, None, None);
    let bytes = vec![127, 0, 0, 1, 0x1a, 0xe1]; // 127.0.0.1:6881
    let peers = client.parse_compact_peers_raw(&bytes).unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].ip, "127.0.0.1");
    assert_eq!(peers[0].port, 6881);
}

#[test]
fn test_parse_non_compact_peers() {
    let client = TrackerClient::new([0; 20], 6881, None, None, None, None);

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

#[tokio::test]
async fn test_bep12_tracker_tiers() {
    // Start mock servers representing trackers
    let server_a = wiremock::MockServer::start().await; // Tier 0 - Tracker A (always succeeds)
    let server_b = wiremock::MockServer::start().await; // Tier 0 - Tracker B (always fails)
    let server_c = wiremock::MockServer::start().await; // Tier 1 - Tracker C (succeeds)

    // Set up mock responders
    // Server A: succeeds
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_bytes(b"d5:peers0:e".to_vec()))
        .mount(&server_a)
        .await;

    // Server B: fails (500)
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&server_b)
        .await;

    // Server C: succeeds
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_bytes(b"d5:peers0:e".to_vec()))
        .mount(&server_c)
        .await;

    let client = TrackerClient::new([0; 20], 6881, None, None, None, None);

    // 1. First scenario: Announce list = [[ServerB (fails)], [ServerC (succeeds)]]
    // The client should try Tier 0 (Server B) -> fails.
    // Then it should fall back to Tier 1 (Server C) -> succeeds.
    let torrent1 = crate::torrent::Torrent {
        announce: "http://example.com/announce".to_string(),
        info: crate::torrent::Info {
            name: "test1".to_string(),
            piece_length: 262144,
            pieces: Some(vec![0; 20]),
            length: Some(1024),
            files: None,
            meta_version: None,
            file_tree: None,
        },
        announce_list: Some(vec![vec![server_b.uri()], vec![server_c.uri()]]),
        comment: None,
        created_by: None,
        creation_date: None,
        piece_layers: None,
    };

    let peers1 = client.announce(&torrent1).await.unwrap();
    assert!(peers1.is_empty());

    // 2. Second scenario: Announce list = [[ServerB (fails), ServerA (succeeds)]]
    // Within Tier 0, Server B fails and Server A succeeds.
    // After announce, Server A should be promoted to the front of Tier 0.
    let torrent2 = crate::torrent::Torrent {
        announce: "http://example.com/announce".to_string(),
        info: crate::torrent::Info {
            name: "test2".to_string(),
            piece_length: 262144,
            pieces: Some(vec![0; 20]),
            length: Some(1024),
            files: None,
            meta_version: None,
            file_tree: None,
        },
        announce_list: Some(vec![vec![server_b.uri(), server_a.uri()]]),
        comment: None,
        created_by: None,
        creation_date: None,
        piece_layers: None,
    };

    let hash2 = torrent2.info_hash_v1().unwrap().unwrap();

    let peers2 = client.announce(&torrent2).await.unwrap();
    assert!(peers2.is_empty());

    // Check cached tiers: Server A should be moved to the front of Tier 0
    let cached = client.tracker_tiers.lock().unwrap();
    let tiers = cached.get(&hash2).unwrap();
    assert_eq!(tiers.len(), 1);
    assert_eq!(tiers[0].len(), 2);
    assert_eq!(tiers[0][0], server_a.uri());
    assert_eq!(tiers[0][1], server_b.uri());
}

#[tokio::test]
async fn test_bep12_tracker_tiers_edge_cases() {
    let client = TrackerClient::new([0; 20], 6881, None, None, None, None);

    let torrent = crate::torrent::Torrent {
        announce: "http://example.com/announce".to_string(),
        info: crate::torrent::Info {
            name: "test_edge".to_string(),
            piece_length: 262144,
            pieces: Some(vec![0; 20]),
            length: Some(1024),
            files: None,
            meta_version: None,
            file_tree: None,
        },
        announce_list: Some(vec![
            vec![
                "http://duplicate.com".to_string(),
                "".to_string(),
                "http://duplicate.com".to_string(),
            ],
            vec![],
            vec![
                "http://duplicate.com".to_string(),
                "http://unique.com".to_string(),
            ],
        ]),
        comment: None,
        created_by: None,
        creation_date: None,
        piece_layers: None,
    };

    let hash = torrent.info_hash_v1().unwrap().unwrap();

    // Call announce - it will fail because these URLs are fake, but it will initialize the cache!
    let _ = client.announce(&torrent).await;

    // Check cached tiers
    let cached = client.tracker_tiers.lock().unwrap();
    let tiers = cached.get(&hash).unwrap();

    // Tiers should be:
    // Tier 0: ["http://duplicate.com"]
    // Tier 1: ["http://unique.com"] (since duplicate was already seen in Tier 0 and empty tier is omitted)
    assert_eq!(tiers.len(), 2);
    assert_eq!(tiers[0], vec!["http://duplicate.com".to_string()]);
    assert_eq!(tiers[1], vec!["http://unique.com".to_string()]);
}
