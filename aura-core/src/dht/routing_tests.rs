use super::*;

#[test]
fn test_node_id_distance() {
    let id1 = [0u8; 20];
    let mut id2 = [0u8; 20];
    id2[19] = 1;

    let rt = RoutingTable::new(id1);
    assert_eq!(rt.distance(&id2), 1);

    let mut id3 = [0u8; 20];
    id3[18] = 1;
    assert!(rt.distance(&id3) > rt.distance(&id2));
}

#[test]
fn test_bucket_index() {
    let id1 = [0u8; 20];
    let mut id2 = [0u8; 20];
    id2[0] = 0x80; // High bit set

    let rt = RoutingTable::new(id1);
    assert_eq!(rt.bucket_index(&id2), 0);

    let mut id3 = [0u8; 20];
    id3[19] = 1;
    assert_eq!(rt.bucket_index(&id3), 159);
}

#[test]
fn test_get_closest_nodes() {
    let my_id = [0u8; 20];
    let mut rt = RoutingTable::new(my_id);

    for i in 1..10 {
        let mut id = [0u8; 20];
        id[19] = i as u8;
        rt.insert(Node {
            id,
            addr: "127.0.0.1:80".parse().unwrap(),
        });
    }

    let target = [0u8; 20];
    let closest = rt.get_closest_nodes(&target, 5);
    assert_eq!(closest.len(), 5);
}
