use super::*;
use std::net::SocketAddr;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_dht_actor_creation() {
    let (_tx, _rx) = mpsc::channel(1);
    let dht = DhtActor::new("127.0.0.1", [0; 20], _rx, None, 0, None).await;
    assert!(dht.is_ok());
}

#[tokio::test]
async fn test_dht_rotating_tokens() {
    let (_tx, _rx) = mpsc::channel(1);
    let dht = DhtActor::new("127.0.0.1", [0; 20], _rx, None, 0, None)
        .await
        .unwrap();
    let addr: SocketAddr = "192.168.1.1:5000".parse().unwrap();

    // Generate token
    let token = dht.generate_token(addr).await;
    assert!(!token.is_empty());

    // Validate token
    let is_valid = dht.validate_token(addr, &token).await;
    assert!(is_valid);

    // Validating with wrong IP should fail
    let wrong_addr: SocketAddr = "192.168.1.2:5000".parse().unwrap();
    let wrong_valid = dht.validate_token(wrong_addr, &token).await;
    assert!(!wrong_valid);

    // Test rotation
    {
        let mut secrets = dht.secrets.lock().await;
        // Simulate time passing (11 minutes ago)
        secrets.last_rotation = std::time::Instant::now() - std::time::Duration::from_secs(660);
    }

    // This generate_token should trigger rotation
    let token2 = dht.generate_token(addr).await;
    assert_ne!(token, token2);

    // Old token should still validate because it matches previous secret
    assert!(dht.validate_token(addr, &token).await);
    // New token should validate
    assert!(dht.validate_token(addr, &token2).await);

    // Rotate again
    {
        let mut secrets = dht.secrets.lock().await;
        secrets.last_rotation = std::time::Instant::now() - std::time::Duration::from_secs(660);
    }

    // Generate new token to rotate secrets again
    let token3 = dht.generate_token(addr).await;

    // Now the original token should be invalid (expired past two rotations)
    assert!(!dht.validate_token(addr, &token).await);
    // Token2 should be valid as the previous secret
    assert!(dht.validate_token(addr, &token2).await);
    // Token3 should be valid as the current secret
    assert!(dht.validate_token(addr, &token3).await);
}

#[tokio::test]
async fn test_dht_persistent_state_serialization() {
    use crate::dht::routing::Node;
    use crate::dht::PersistentState;
    use tempfile::NamedTempFile;

    let (_tx, _rx) = mpsc::channel(1);
    let dht = DhtActor::new("127.0.0.1", [1; 20], _rx, None, 0, None)
        .await
        .unwrap();

    let node_id = [2u8; 20];
    let node_addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();

    // Insert a node into the routing table
    {
        let mut rt = dht.routing_table.lock().await;
        rt.insert(Node {
            id: node_id,
            addr: node_addr,
        });
    }

    // Create a temp file path
    let temp_file = NamedTempFile::new().unwrap();
    let file_path = temp_file.path();

    // Save state
    dht.save(file_path).await.unwrap();

    // Create a new actor and load state
    let (_tx2, _rx2) = mpsc::channel(1);
    let mut dht2 = DhtActor::new("127.0.0.1", [1; 20], _rx2, None, 0, None)
        .await
        .unwrap();

    // Load state
    dht2.load(file_path).await.unwrap();

    // Verify node is present in the new routing table
    {
        let rt = dht2.routing_table.lock().await;
        let closest = rt.get_closest_nodes(&node_id, 1);
        assert_eq!(closest.len(), 1);
        assert_eq!(closest[0].id, node_id);
        assert_eq!(closest[0].addr, node_addr);
    }
}

#[tokio::test]
async fn test_dht_save_now_command() {
    use crate::dht::routing::Node;
    use tempfile::tempdir;
    use tokio::sync::oneshot;

    let (tx, rx) = mpsc::channel(1);

    let tmp_dir = tempdir().unwrap();
    std::env::set_var("HOME", tmp_dir.path());
    std::env::set_var("USERPROFILE", tmp_dir.path());

    let dht = DhtActor::new("127.0.0.1", [1; 20], rx, None, 0, None)
        .await
        .unwrap();

    let node_id = [2u8; 20];
    let node_addr: SocketAddr = "127.0.0.1:9999".parse().unwrap();
    {
        let mut rt = dht.routing_table.lock().await;
        rt.insert(Node {
            id: node_id,
            addr: node_addr,
        });
    }

    let handle = tokio::spawn(async move {
        let _ = dht.run().await;
    });

    let (reply_tx, reply_rx) = oneshot::channel();
    tx.send(DhtCommand::SaveNow(reply_tx)).await.unwrap();

    reply_rx.await.unwrap();

    let mut expected_path = tmp_dir.path().to_path_buf();
    expected_path.push(".aura");
    expected_path.push("dht.dat");
    assert!(expected_path.exists());

    handle.abort();
}
