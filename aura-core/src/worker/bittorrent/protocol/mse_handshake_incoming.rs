use super::mse::{fill_random_bytes, get_random_range, hash_sha1, DiffieHellman, MseStream, Rc4};
use crate::{Error, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_PADDING_LEN: usize = 512;

impl<S> MseStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub async fn handshake_incoming(
        &mut self,
        ya: [u8; 96],
        policy: crate::config::EncryptionPolicy,
        active_torrents: &[crate::InfoHash],
    ) -> Result<(crate::InfoHash, Vec<u8>)> {
        // 2. Generate local DH key pair
        let dh = DiffieHellman::new();
        let yb_bytes = dh.y.to_bytes_be();
        let mut yb = vec![0u8; 96];
        if yb_bytes.len() <= 96 {
            yb[96 - yb_bytes.len()..].copy_from_slice(&yb_bytes);
        } else {
            return Err(Error::Protocol("DH public key is too large".to_string()));
        }

        // 3. Generate random PadB (0 to 512 bytes)
        let pad_b_len = get_random_range(0, MAX_PADDING_LEN);
        let mut pad_b = vec![0u8; pad_b_len];
        fill_random_bytes(&mut pad_b);

        // 4. Send Yb + PadB
        let mut first_payload = Vec::with_capacity(96 + pad_b_len);
        first_payload.extend_from_slice(&yb);
        first_payload.extend_from_slice(&pad_b);
        self.inner
            .write_all(&first_payload)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        // 5. Compute shared secret S
        let s = dh.compute_shared_secret(&ya)?;

        // 6. Synchronize on HASH('req1', S)
        let expected_req1 = hash_sha1(b"req1", &s, None);
        let mut window = Vec::new();
        let mut found_sync = false;
        let read_limit = MAX_PADDING_LEN + 20;

        for _ in 0..read_limit {
            let mut byte_buf = [0u8; 1];
            self.inner
                .read_exact(&mut byte_buf)
                .await
                .map_err(|e| Error::Protocol(format!("Failed to read sync byte: {}", e)))?;
            window.push(byte_buf[0]);
            if window.len() >= 20 {
                let current_window = &window[window.len() - 20..];
                if current_window == expected_req1 {
                    found_sync = true;
                    break;
                }
            }
        }

        if !found_sync {
            return Err(Error::Protocol(
                "Failed to synchronize on HASH('req1', S)".to_string(),
            ));
        }

        // 7. Read HASH('req2', SKEY) XOR HASH('req3', S) (20 bytes)
        let mut hash_skey_xor = [0u8; 20];
        self.inner
            .read_exact(&mut hash_skey_xor)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        // 8. Retrieve expected_hash_req2 by XORing with HASH('req3', S)
        let hash_req3 = hash_sha1(b"req3", &s, None);
        let mut expected_hash_req2 = [0u8; 20];
        for i in 0..20 {
            expected_hash_req2[i] = hash_skey_xor[i] ^ hash_req3[i];
        }

        // 9. Find matched info_hash
        let mut matched_info_hash = None;
        for ih in active_torrents {
            let test_hash = hash_sha1(b"req2", &ih.for_handshake(), None);
            if test_hash == expected_hash_req2 {
                matched_info_hash = Some(*ih);
                break;
            }
        }

        let info_hash = match matched_info_hash {
            Some(ih) => ih,
            None => {
                return Err(Error::Protocol(
                    "Incoming peer requested unknown info_hash".to_string(),
                ))
            }
        };

        // 10. Initialize decryptor keyA and encryptor keyB
        let info_hash_bytes = info_hash.for_handshake();
        let key_a = hash_sha1(b"keyA", &s, Some(&info_hash_bytes));
        let key_b = hash_sha1(b"keyB", &s, Some(&info_hash_bytes));

        let mut dec = Rc4::new(&key_a);
        let mut enc = Rc4::new(&key_b);

        let mut discard = [0u8; 1024];
        dec.process(&mut discard);
        enc.process(&mut discard);

        // 11. Decrypt vc (8 bytes), crypto_provide (4 bytes), len(PadC) (2 bytes)
        let mut vc = [0u8; 8];
        self.inner
            .read_exact(&mut vc)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        dec.process(&mut vc);
        if vc != [0u8; 8] {
            return Err(Error::Protocol(
                "Verification constant mismatch".to_string(),
            ));
        }

        let mut crypto_provide_bytes = [0u8; 4];
        self.inner
            .read_exact(&mut crypto_provide_bytes)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        dec.process(&mut crypto_provide_bytes);
        let crypto_provide = u32::from_be_bytes(crypto_provide_bytes);

        let mut pad_c_len_bytes = [0u8; 2];
        self.inner
            .read_exact(&mut pad_c_len_bytes)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        dec.process(&mut pad_c_len_bytes);
        let pad_c_len = u16::from_be_bytes(pad_c_len_bytes) as usize;

        if pad_c_len > MAX_PADDING_LEN {
            return Err(Error::Protocol(format!(
                "Invalid PadC length: {}",
                pad_c_len
            )));
        }

        // Read and decrypt PadC
        if pad_c_len > 0 {
            let mut pad_c = vec![0u8; pad_c_len];
            self.inner
                .read_exact(&mut pad_c)
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?;
            dec.process(&mut pad_c);
        }

        // Read len(IA) (2 bytes)
        let mut ia_len_bytes = [0u8; 2];
        self.inner
            .read_exact(&mut ia_len_bytes)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;
        dec.process(&mut ia_len_bytes);
        let ia_len = u16::from_be_bytes(ia_len_bytes) as usize;

        if ia_len > 4096 {
            return Err(Error::Protocol(format!(
                "Initial payload (IA) length too large: {}",
                ia_len
            )));
        }

        // Read and decrypt IA
        let mut ia = vec![0u8; ia_len];
        if ia_len > 0 {
            self.inner
                .read_exact(&mut ia)
                .await
                .map_err(|e| Error::Protocol(e.to_string()))?;
            dec.process(&mut ia);
        }

        // 12. Decide crypto_select
        let crypto_select: u32 = if (crypto_provide & 0x02) != 0
            && policy != crate::config::EncryptionPolicy::Disable
        {
            0x02 // Choose RC4
        } else if (crypto_provide & 0x01) != 0 && policy != crate::config::EncryptionPolicy::Require
        {
            0x01 // Choose Plaintext
        } else {
            return Err(Error::Protocol(
                "No mutually agreeable encryption method".to_string(),
            ));
        };

        // 13. Send ENCRYPT(vc + crypto_select + len(PadD) + PadD)
        let pad_d_len = get_random_range(0, MAX_PADDING_LEN);
        let mut pad_d = vec![0u8; pad_d_len];
        fill_random_bytes(&mut pad_d);

        let mut response = Vec::new();
        let vc_out = [0u8; 8];
        response.extend_from_slice(&vc_out);
        response.extend_from_slice(&crypto_select.to_be_bytes());
        response.extend_from_slice(&(pad_d_len as u16).to_be_bytes());
        response.extend_from_slice(&pad_d);

        // Encrypt response using B's encryptor keyB
        enc.process(&mut response);
        self.inner
            .write_all(&response)
            .await
            .map_err(|e| Error::Protocol(e.to_string()))?;

        // 14. If selected RC4, initialize our stream ciphers
        if crypto_select == 0x02 {
            self.encryptor = Some(enc);
            self.decryptor = Some(dec);
        }

        Ok((info_hash, ia))
    }
}
