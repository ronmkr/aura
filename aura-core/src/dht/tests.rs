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
