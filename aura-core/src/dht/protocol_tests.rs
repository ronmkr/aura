use super::*;
use std::collections::BTreeMap;

#[test]
fn test_krpc_serialization() {
    let mut args = BTreeMap::new();
    args.insert(
        "id".to_string(),
        serde_bencode::value::Value::Bytes(vec![0; 20]),
    );

    let msg = KrpcMessage {
        transaction_id: vec![1, 2, 3],
        msg_type: "q".to_string(),
        query: Some("ping".to_string()),
        args: Some(args),
        response: None,
        error: None,
    };

    let encoded = msg.encode().unwrap();
    let decoded = KrpcMessage::decode(&encoded).unwrap();
    assert_eq!(msg.transaction_id, decoded.transaction_id);
    assert_eq!(msg.query, decoded.query);
}
