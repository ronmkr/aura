use super::*;

#[test]
fn test_torrent_serialization() {
    let info = Info {
        name: "test.txt".to_string(),
        piece_length: 1024,
        pieces: Some(vec![0; 20]),
        length: Some(1024),
        files: None,
        meta_version: None,
        file_tree: None,
    };
    let torrent = Torrent {
        announce: "http://tracker.com/announce".to_string(),
        info,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        piece_layers: None,
    };

    let encoded = serde_bencode::to_bytes(&torrent).unwrap();
    let decoded = Torrent::from_bytes(&encoded).unwrap();

    assert_eq!(decoded.announce, "http://tracker.com/announce");
    assert_eq!(decoded.info.name, "test.txt");
    assert_eq!(decoded.total_length(), 1024);

    let hash = decoded.info_hash_v1().unwrap().unwrap();
    assert_eq!(hash.len(), 20);
}

#[test]
fn test_flatten_v2_files() {
    use serde_bencode::value::Value;
    use std::collections::HashMap;

    let mut file1_props = HashMap::new();
    file1_props.insert(b"length".to_vec(), Value::Int(100));
    file1_props.insert(b"pieces root".to_vec(), Value::Bytes(vec![1; 32]));

    let mut file1_entry = HashMap::new();
    file1_entry.insert(b"".to_vec(), Value::Dict(file1_props));

    let mut file2_props = HashMap::new();
    file2_props.insert(b"length".to_vec(), Value::Int(200));
    file2_props.insert(b"pieces root".to_vec(), Value::Bytes(vec![2; 32]));

    let mut file2_entry = HashMap::new();
    file2_entry.insert(b"".to_vec(), Value::Dict(file2_props));

    let mut dir2 = HashMap::new();
    dir2.insert(b"file2.txt".to_vec(), Value::Dict(file2_entry));

    let mut dir1 = HashMap::new();
    dir1.insert(b"file1.txt".to_vec(), Value::Dict(file1_entry));
    dir1.insert(b"dir2".to_vec(), Value::Dict(dir2));

    let mut file_tree = HashMap::new();
    file_tree.insert(b"dir1".to_vec(), Value::Dict(dir1));

    let info = Info {
        name: "test".to_string(),
        piece_length: 1024,
        pieces: None,
        length: None,
        files: None,
        meta_version: Some(2),
        file_tree: Some(Value::Dict(file_tree)),
    };

    let torrent = Torrent {
        announce: "http://tracker.com/announce".to_string(),
        info,
        announce_list: None,
        comment: None,
        created_by: None,
        creation_date: None,
        piece_layers: None,
    };

    assert_eq!(torrent.total_length(), 300);

    let v2_files = torrent.flatten_v2_files().unwrap();
    assert_eq!(v2_files.len(), 2);

    let f1 = v2_files
        .iter()
        .find(|f| f.path.last().unwrap() == "file1.txt")
        .unwrap();
    assert_eq!(f1.path, vec!["dir1".to_string(), "file1.txt".to_string()]);
    assert_eq!(f1.length, 100);
    assert_eq!(f1.pieces_root.as_ref().unwrap(), &vec![1; 32]);

    let f2 = v2_files
        .iter()
        .find(|f| f.path.last().unwrap() == "file2.txt")
        .unwrap();
    assert_eq!(
        f2.path,
        vec![
            "dir1".to_string(),
            "dir2".to_string(),
            "file2.txt".to_string()
        ]
    );
    assert_eq!(f2.length, 200);
    assert_eq!(f2.pieces_root.as_ref().unwrap(), &vec![2; 32]);
}
