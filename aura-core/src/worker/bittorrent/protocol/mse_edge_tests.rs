use super::mse::MseStream;
use crate::config::EncryptionPolicy;
use crate::InfoHash;

#[tokio::test]
async fn test_mse_handshake_failure_unknown_info_hash() {
    let (client, server) = tokio::io::duplex(1024);
    let mut client_mse = MseStream::new(client);
    let server_mse = MseStream::new(server);

    let client_ih = InfoHash::V1([0x11; 20]);
    let server_ih = InfoHash::V1([0x22; 20]);
    let active_torrents = vec![server_ih];
    let ia_payload = b"BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x11\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14";

    let client_handle = tokio::spawn(async move {
        client_mse
            .handshake_outgoing(
                &client_ih.for_handshake(),
                EncryptionPolicy::Prefer,
                ia_payload,
            )
            .await
    });

    let server_handle = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut first_byte = [0u8; 1];
        let mut remaining = [0u8; 95];
        let mut inner = server_mse.inner;
        inner
            .read_exact(&mut first_byte)
            .await
            .map_err(|e| crate::Error::Protocol(e.to_string()))?;
        inner
            .read_exact(&mut remaining)
            .await
            .map_err(|e| crate::Error::Protocol(e.to_string()))?;
        let mut ya = [0u8; 96];
        ya[0] = first_byte[0];
        ya[1..].copy_from_slice(&remaining);

        let mut server_mse = MseStream::new(inner);
        server_mse
            .handshake_incoming(ya, EncryptionPolicy::Prefer, &active_torrents)
            .await
    });

    let (client_res, server_res) = tokio::join!(client_handle, server_handle);
    let client_res = client_res.unwrap();
    let server_res = server_res.unwrap();

    assert!(
        server_res.is_err(),
        "Server should have failed: {:?}",
        server_res
    );
    assert!(
        client_res.is_err(),
        "Client should have failed: {:?}",
        client_res
    );
}

#[tokio::test]
async fn test_mse_handshake_failure_policy_mismatch() {
    let (client, server) = tokio::io::duplex(1024);
    let mut client_mse = MseStream::new(client);
    let server_mse = MseStream::new(server);

    let info_hash = InfoHash::V1([0x33; 20]);
    let active_torrents = vec![info_hash];
    let ia_payload = b"BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x33\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14";

    let client_handle = tokio::spawn(async move {
        client_mse
            .handshake_outgoing(
                &info_hash.for_handshake(),
                EncryptionPolicy::Disable,
                ia_payload,
            )
            .await
    });

    let server_handle = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut first_byte = [0u8; 1];
        let mut remaining = [0u8; 95];
        let mut inner = server_mse.inner;
        inner
            .read_exact(&mut first_byte)
            .await
            .map_err(|e| crate::Error::Protocol(e.to_string()))?;
        inner
            .read_exact(&mut remaining)
            .await
            .map_err(|e| crate::Error::Protocol(e.to_string()))?;
        let mut ya = [0u8; 96];
        ya[0] = first_byte[0];
        ya[1..].copy_from_slice(&remaining);

        let mut server_mse = MseStream::new(inner);
        server_mse
            .handshake_incoming(ya, EncryptionPolicy::Require, &active_torrents)
            .await
    });

    let (client_res, server_res) = tokio::join!(client_handle, server_handle);
    let client_res = client_res.unwrap();
    let server_res = server_res.unwrap();

    assert!(
        server_res.is_err(),
        "Server should have failed due to policy mismatch: {:?}",
        server_res
    );
    assert!(
        client_res.is_err(),
        "Client should have failed: {:?}",
        client_res
    );
}

#[tokio::test]
async fn test_mse_handshake_failure_policy_mismatch_client_side() {
    let (client, server) = tokio::io::duplex(1024);
    let mut client_mse = MseStream::new(client);
    let server_mse = MseStream::new(server);

    let info_hash = InfoHash::V1([0x44; 20]);
    let active_torrents = vec![info_hash];
    let ia_payload = b"BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x44\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14";

    let client_handle = tokio::spawn(async move {
        client_mse
            .handshake_outgoing(
                &info_hash.for_handshake(),
                EncryptionPolicy::Require,
                ia_payload,
            )
            .await
    });

    let server_handle = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut first_byte = [0u8; 1];
        let mut remaining = [0u8; 95];
        let mut inner = server_mse.inner;
        inner
            .read_exact(&mut first_byte)
            .await
            .map_err(|e| crate::Error::Protocol(e.to_string()))?;
        inner
            .read_exact(&mut remaining)
            .await
            .map_err(|e| crate::Error::Protocol(e.to_string()))?;
        let mut ya = [0u8; 96];
        ya[0] = first_byte[0];
        ya[1..].copy_from_slice(&remaining);

        let mut server_mse = MseStream::new(inner);
        server_mse
            .handshake_incoming(ya, EncryptionPolicy::Disable, &active_torrents)
            .await
    });

    let (client_res, server_res) = tokio::join!(client_handle, server_handle);
    let client_res = client_res.unwrap();
    let server_res = server_res.unwrap();

    assert!(
        client_res.is_err(),
        "Client should have failed due to policy mismatch: {:?}",
        client_res
    );
    assert!(
        server_res.is_err(),
        "Server should have failed: {:?}",
        server_res
    );
}

#[tokio::test]
async fn test_mse_handshake_failure_sync() {
    let (client, mut server) = tokio::io::duplex(1024);
    let mut client_mse = MseStream::new(client);

    let info_hash = InfoHash::V1([0x55; 20]);
    let ia_payload = b"BitTorrent protocol\x00\x00\x00\x00\x00\x00\x00\x00\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x55\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10\x11\x12\x13\x14";

    let client_handle = tokio::spawn(async move {
        client_mse
            .handshake_outgoing(
                &info_hash.for_handshake(),
                EncryptionPolicy::Prefer,
                ia_payload,
            )
            .await
    });

    use tokio::io::AsyncWriteExt;
    let mut garbage = vec![0u8; 600];
    super::mse::fill_random_bytes(&mut garbage);
    server.write_all(&garbage).await.unwrap();
    drop(server);

    let client_res = client_handle.await.unwrap();
    assert!(
        client_res.is_err(),
        "Client should have failed to sync: {:?}",
        client_res
    );
}

#[tokio::test]
async fn test_mse_handshake_failure_invalid_dh_key() {
    let (_client, server) = tokio::io::duplex(1024);
    let mut server_mse = MseStream::new(server);
    let active_torrents = vec![InfoHash::V1([0x66; 20])];

    // ya is 96 bytes of 0 (invalid DH public key)
    let ya = [0u8; 96];
    let res = server_mse
        .handshake_incoming(ya, EncryptionPolicy::Prefer, &active_torrents)
        .await;
    assert!(
        res.is_err(),
        "Incoming handshake should fail on invalid DH key"
    );
}

#[tokio::test]
async fn test_mse_handshake_failure_ia_payload_too_large() {
    let (client, server) = tokio::io::duplex(2048);
    let mut client_mse = MseStream::new(client);
    let mut server_mse = MseStream::new(server);

    let info_hash = InfoHash::V1([0x77; 20]);
    let active_torrents = vec![info_hash];

    let client_handle = tokio::spawn(async move {
        use tokio::io::AsyncWriteExt;

        let dh = super::mse::DiffieHellman::new();
        let ya_bytes = dh.y.to_bytes_be();
        let mut ya = vec![0u8; 96];
        if ya_bytes.len() <= 96 {
            ya[96 - ya_bytes.len()..].copy_from_slice(&ya_bytes);
        }

        // Write Ya
        client_mse.inner.write_all(&ya).await.unwrap();

        // Read Yb (96 bytes)
        let mut yb = [0u8; 96];
        use tokio::io::AsyncReadExt;
        client_mse.inner.read_exact(&mut yb).await.unwrap();

        let s = dh.compute_shared_secret(&yb).unwrap();

        let hash_req1 = super::mse::hash_sha1(b"req1", &s, None);
        let hash_req2 = super::mse::hash_sha1(b"req2", &info_hash.for_handshake(), None);
        let hash_req3 = super::mse::hash_sha1(b"req3", &s, None);
        let mut hash_skey_xor = [0u8; 20];
        for i in 0..20 {
            hash_skey_xor[i] = hash_req2[i] ^ hash_req3[i];
        }

        let key_a = super::mse::hash_sha1(b"keyA", &s, Some(&info_hash.for_handshake()));
        // codeql[rust/weak-cryptographic-algorithm]
        let mut enc = super::mse::Rc4::new(&key_a);
        let mut discard = [0u8; 1024];
        enc.process(&mut discard);

        // Construct encrypted part:
        // vc (8 bytes) + crypto_provide (4 bytes) + len(PadC) (2 bytes) + PadC (0 bytes) + len(IA) (2 bytes, set to 5000)
        let vc = [0u8; 8];
        let crypto_provide: u32 = 2; // RC4 only
        let pad_c_len: u16 = 0;
        let ia_len: u16 = 5000; // invalid!

        let mut encrypted = Vec::new();
        encrypted.extend_from_slice(&vc);
        encrypted.extend_from_slice(&crypto_provide.to_be_bytes());
        encrypted.extend_from_slice(&pad_c_len.to_be_bytes());
        encrypted.extend_from_slice(&ia_len.to_be_bytes());

        enc.process(&mut encrypted);

        let mut payload = Vec::new();
        payload.extend_from_slice(&hash_req1);
        payload.extend_from_slice(&hash_skey_xor);
        payload.extend_from_slice(&encrypted);

        client_mse.inner.write_all(&payload).await.unwrap();

        // Keep connection open momentarily
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    });

    let server_handle = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut ya = [0u8; 96];
        server_mse.inner.read_exact(&mut ya).await.unwrap();
        server_mse
            .handshake_incoming(ya, EncryptionPolicy::Require, &active_torrents)
            .await
    });

    let (client_res, server_res) = tokio::join!(client_handle, server_handle);
    client_res.unwrap();
    let server_res = server_res.unwrap();

    assert!(
        server_res.is_err(),
        "Server should have rejected large IA payload"
    );
    let err_str = server_res.err().unwrap().to_string();
    assert!(
        err_str.contains("Initial payload (IA) length too large"),
        "Unexpected error: {}",
        err_str
    );
}
