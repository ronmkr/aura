use super::mse::{DiffieHellman, MseStream, Rc4};
use crate::config::EncryptionPolicy;
use crate::InfoHash;

#[test]
fn test_rc4_obfuscation() {
    let key = b"test_secret_key";
    let mut cipher1 = Rc4::new(key);
    let mut cipher2 = Rc4::new(key);

    let mut data = b"Hello, BitTorrent Obfuscation!".to_vec();
    let original = data.clone();

    // Encrypt
    cipher1.process(&mut data);
    assert_ne!(data, original); // Ciphertext should be different

    // Decrypt
    cipher2.process(&mut data);
    assert_eq!(data, original); // Decrypted should be same
}

#[test]
fn test_diffie_hellman() {
    let dh_a = DiffieHellman::new();
    let dh_b = DiffieHellman::new();

    let ya_bytes = dh_a.y.to_bytes_be();
    let yb_bytes = dh_b.y.to_bytes_be();

    let secret_a = dh_a.compute_shared_secret(&yb_bytes).unwrap();
    let secret_b = dh_b.compute_shared_secret(&ya_bytes).unwrap();

    assert_eq!(secret_a, secret_b);
    assert_eq!(secret_a.len(), 96);
}

#[tokio::test]
async fn test_mse_handshake_success_rc4() {
    let (client, server) = tokio::io::duplex(1024);
    let mut client_mse = MseStream::new(client);
    let server_mse = MseStream::new(server);

    let info_hash = InfoHash::V1([0xAB; 20]);
    let active_torrents = vec![info_hash];
    let ia_payload = b"BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\xab\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14"; // 68 bytes

    let client_handle = tokio::spawn(async move {
        client_mse
            .handshake_outgoing(
                &info_hash.for_handshake(),
                EncryptionPolicy::Prefer,
                ia_payload,
            )
            .await?;
        Ok::<_, crate::Error>(client_mse)
    });

    let server_handle = tokio::spawn(async move {
        // Reconstruct incoming ya pre-read mock behavior
        use tokio::io::AsyncReadExt;
        let mut first_byte = [0u8; 1];
        let mut remaining = [0u8; 95];
        let mut inner = server_mse.inner;
        inner.read_exact(&mut first_byte).await.unwrap();
        inner.read_exact(&mut remaining).await.unwrap();
        let mut ya = [0u8; 96];
        ya[0] = first_byte[0];
        ya[1..].copy_from_slice(&remaining);

        // Put inner back into mse stream
        let mut server_mse = MseStream::new(inner);

        let (matched_ih, decrypted_ia) = server_mse
            .handshake_incoming(ya, EncryptionPolicy::Prefer, &active_torrents)
            .await?;
        Ok::<_, crate::Error>((server_mse, matched_ih, decrypted_ia))
    });

    let client_res = client_handle.await.unwrap();
    let server_res = server_handle.await.unwrap();

    assert!(client_res.is_ok(), "Client error: {:?}", client_res);
    assert!(server_res.is_ok(), "Server error: {:?}", server_res);

    let (mut client_mse, (mut server_mse, matched_ih, decrypted_ia)) =
        (client_res.unwrap(), server_res.unwrap());

    assert_eq!(matched_ih, info_hash);
    assert_eq!(decrypted_ia, ia_payload);

    // Verify that subsequent communication is encrypted and works
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    client_mse.write_all(b"Ping").await.unwrap();
    client_mse.flush().await.unwrap();
    let mut buf = [0u8; 4];
    server_mse.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"Ping");

    server_mse.write_all(b"Pong").await.unwrap();
    server_mse.flush().await.unwrap();
    let mut buf2 = [0u8; 4];
    client_mse.read_exact(&mut buf2).await.unwrap();
    assert_eq!(&buf2, b"Pong");
}

#[tokio::test]
async fn test_mse_handshake_fallback_plaintext() {
    let (client, server) = tokio::io::duplex(1024);
    let mut client_mse = MseStream::new(client);
    let server_mse = MseStream::new(server);

    let info_hash = InfoHash::V1([0xDE; 20]);
    let active_torrents = vec![info_hash];
    let ia_payload = b"BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\xde\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14";

    let client_handle = tokio::spawn(async move {
        client_mse
            .handshake_outgoing(
                &info_hash.for_handshake(),
                EncryptionPolicy::Prefer,
                ia_payload,
            )
            .await?;
        Ok::<_, crate::Error>(client_mse)
    });

    let server_handle = tokio::spawn(async move {
        // Reconstruct incoming ya pre-read mock behavior
        use tokio::io::AsyncReadExt;
        let mut first_byte = [0u8; 1];
        let mut remaining = [0u8; 95];
        let mut inner = server_mse.inner;
        inner.read_exact(&mut first_byte).await.unwrap();
        inner.read_exact(&mut remaining).await.unwrap();
        let mut ya = [0u8; 96];
        ya[0] = first_byte[0];
        ya[1..].copy_from_slice(&remaining);

        // Put inner back into mse stream
        let mut server_mse = MseStream::new(inner);

        let (matched_ih, decrypted_ia) = server_mse
            .handshake_incoming(ya, EncryptionPolicy::Disable, &active_torrents)
            .await?;
        Ok::<_, crate::Error>((server_mse, matched_ih, decrypted_ia))
    });

    let client_res = client_handle.await.unwrap();
    let server_res = server_handle.await.unwrap();

    assert!(client_res.is_ok(), "Client error: {:?}", client_res);
    assert!(server_res.is_ok(), "Server error: {:?}", server_res);

    let (mut client_mse, (mut server_mse, matched_ih, decrypted_ia)) =
        (client_res.unwrap(), server_res.unwrap());

    assert_eq!(matched_ih, info_hash);
    assert_eq!(decrypted_ia, ia_payload);

    // Verify that subsequent communication is plaintext and works
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    client_mse.write_all(b"Hello").await.unwrap();
    client_mse.flush().await.unwrap();
    let mut buf = [0u8; 5];
    server_mse.read_exact(&mut buf).await.unwrap();
    assert_eq!(&buf, b"Hello");
}
