use super::mse::{fill_random_bytes, get_random_range, hash_sha1, DiffieHellman, MseStream, Rc4};
use crate::{Error, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_PADDING_LEN: usize = 512;

impl<S> MseStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub async fn handshake_outgoing(
        &mut self,
        info_hash: &[u8; 20],
        policy: crate::config::EncryptionPolicy,
        ia: &[u8],
    ) -> Result<()> {
        // 1. Generate DH key pair
        let dh = DiffieHellman::new();
        let ya_bytes = dh.y.to_bytes_be();
        let mut ya = vec![0u8; 96];
        if ya_bytes.len() <= 96 {
            ya[96 - ya_bytes.len()..].copy_from_slice(&ya_bytes);
        } else {
            return Err(Error::Protocol("DH public key is too large".to_string()));
        }

        // 2. Generate random PadA (0 to 512 bytes)
        let pad_a_len = get_random_range(0, MAX_PADDING_LEN);
        let mut pad_a = vec![0u8; pad_a_len];
        fill_random_bytes(&mut pad_a);

        // 3. Send Ya + PadA
        let mut first_payload = Vec::with_capacity(96 + pad_a_len);
        first_payload.extend_from_slice(&ya);
        first_payload.extend_from_slice(&pad_a);
        self.inner
            .write_all(&first_payload)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        // 4. Read Yb (96 bytes)
        let mut yb = [0u8; 96];
        self.inner
            .read_exact(&mut yb)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        // 5. Compute shared secret S
        let s = dh.compute_shared_secret(&yb)?;

        // 6. Send HASH('req1', S) + HASH('req2', SKEY) XOR HASH('req3', S) + ENCRYPT(...)
        let hash_req1 = hash_sha1(b"req1", &s, None);
        let hash_req2 = hash_sha1(b"req2", info_hash, None);
        let hash_req3 = hash_sha1(b"req3", &s, None);
        let mut hash_skey_xor = [0u8; 20];
        for i in 0..20 {
            hash_skey_xor[i] = hash_req2[i] ^ hash_req3[i];
        }

        // Derive RC4 keys
        let key_a = hash_sha1(b"keyA", &s, Some(info_hash));
        let key_b = hash_sha1(b"keyB", &s, Some(info_hash));

        // Initialize RC4 encryptor and decryptor
        let mut enc = Rc4::new(&key_a);
        let mut dec = Rc4::new(&key_b);

        // Discard first 1024 bytes of keystream for both ciphers
        let mut discard = [0u8; 1024];
        enc.process(&mut discard);
        dec.process(&mut discard);

        // Prepare encrypted part
        let vc = [0u8; 8];
        let crypto_provide: u32 = match policy {
            crate::config::EncryptionPolicy::Require => 0x02, // RC4 only
            crate::config::EncryptionPolicy::Disable => 0x01, // Plaintext only
            crate::config::EncryptionPolicy::Prefer => 0x03,  // RC4 & Plaintext
        };
        let pad_c_len = get_random_range(0, MAX_PADDING_LEN);
        let mut pad_c = vec![0u8; pad_c_len];
        fill_random_bytes(&mut pad_c);

        let ia_len = ia.len() as u16;

        let mut encrypted_payload = Vec::new();
        encrypted_payload.extend_from_slice(&vc);
        encrypted_payload.extend_from_slice(&crypto_provide.to_be_bytes());
        encrypted_payload.extend_from_slice(&(pad_c_len as u16).to_be_bytes());
        encrypted_payload.extend_from_slice(&pad_c);
        encrypted_payload.extend_from_slice(&ia_len.to_be_bytes());
        encrypted_payload.extend_from_slice(ia);

        // Encrypt the payload using enc
        enc.process(&mut encrypted_payload);

        let mut second_payload = Vec::with_capacity(40 + encrypted_payload.len());
        second_payload.extend_from_slice(&hash_req1);
        second_payload.extend_from_slice(&hash_skey_xor);
        second_payload.extend_from_slice(&encrypted_payload);

        self.inner
            .write_all(&second_payload)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        // 7. Synchronize on B's response
        let mut sync_dec = dec.clone();
        let mut expected_vc = [0u8; 8];
        sync_dec.process(&mut expected_vc);

        let mut found_sync = false;
        let mut window = Vec::with_capacity(8);
        let read_limit = MAX_PADDING_LEN + 8;

        for _ in 0..read_limit {
            let mut byte_buf = [0u8; 1];
            self.inner
                .read_exact(&mut byte_buf)
                .await
                .map_err(|e| Error::Protocol(format!("Failed to read sync byte: {}", e)))?;

            if window.len() >= 8 {
                window.remove(0);
            }
            window.push(byte_buf[0]);

            if window.len() == 8 && window.as_slice() == &expected_vc[..] {
                found_sync = true;
                break;
            }
        }

        if !found_sync {
            return Err(Error::Protocol(
                "Failed to synchronize on receiver VC".to_string(),
            ));
        }

        dec = sync_dec;

        // Read 4 bytes of crypto_select
        let mut crypto_select_bytes = [0u8; 4];
        self.inner
            .read_exact(&mut crypto_select_bytes)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        dec.process(&mut crypto_select_bytes);
        let crypto_select = u32::from_be_bytes(crypto_select_bytes);

        if crypto_select != 1 && crypto_select != 2 {
            return Err(Error::Protocol(format!(
                "Invalid crypto_select value: {}",
                crypto_select
            )));
        }

        if policy == crate::config::EncryptionPolicy::Require && crypto_select == 1 {
            return Err(Error::Protocol(
                "Encryption required but peer selected plaintext".to_string(),
            ));
        }

        if policy == crate::config::EncryptionPolicy::Disable && crypto_select == 2 {
            return Err(Error::Protocol(
                "Encryption disabled but peer selected RC4".to_string(),
            ));
        }

        // Read len(PadD) (2 bytes)
        let mut pad_d_len_bytes = [0u8; 2];
        self.inner
            .read_exact(&mut pad_d_len_bytes)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        dec.process(&mut pad_d_len_bytes);
        let pad_d_len = u16::from_be_bytes(pad_d_len_bytes) as usize;

        if pad_d_len > MAX_PADDING_LEN {
            return Err(Error::Protocol(format!(
                "Invalid PadD length: {}",
                pad_d_len
            )));
        }

        // Read PadD bytes
        if pad_d_len > 0 {
            let mut pad_d = vec![0u8; pad_d_len];
            self.inner
                .read_exact(&mut pad_d)
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?;
            dec.process(&mut pad_d);
        }

        // If crypto_select is 2 (RC4), we store the encryptor and decryptor.
        if crypto_select == 2 {
            self.encryptor = Some(enc);
            self.decryptor = Some(dec);
        }

        Ok(())
    }
}
