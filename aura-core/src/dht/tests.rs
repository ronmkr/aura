use super::*;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_dht_actor_creation() {
    let (_tx, _rx) = mpsc::channel(1);
    let dht = DhtActor::new("127.0.0.1", [0; 20], _rx, None, 0, None).await;
    assert!(dht.is_ok());
}
