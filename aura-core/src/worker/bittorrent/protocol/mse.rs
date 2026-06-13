use num_bigint::BigUint;
use rand::{Rng, RngExt};
use sha1::{Digest, Sha1};
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub fn get_random_range(low: usize, high: usize) -> usize {
    let mut rng = rand::rng();
    rng.random_range(low..=high)
}

pub fn fill_random_bytes(buf: &mut [u8]) {
    let mut rng = rand::rng();
    rng.fill(buf);
}

#[derive(Debug, Clone)]
pub struct Rc4 {
    s: [u8; 256],
    i: u8,
    j: u8,
}

impl Rc4 {
    pub fn new(key: &[u8]) -> Self {
        let mut s = [0u8; 256];
        for (i, val) in s.iter_mut().enumerate() {
            *val = i as u8;
        }
        let mut j: u8 = 0;
        for i in 0..256 {
            let val = s[i];
            j = j.wrapping_add(val).wrapping_add(key[i % key.len()]);
            s[i] = s[j as usize];
            s[j as usize] = val;
        }
        Self { s, i: 0, j: 0 }
    }

    pub fn process(&mut self, data: &mut [u8]) {
        for byte in data.iter_mut() {
            self.i = self.i.wrapping_add(1);
            let val_i = self.s[self.i as usize];
            self.j = self.j.wrapping_add(val_i);
            let val_j = self.s[self.j as usize];
            self.s[self.i as usize] = val_j;
            self.s[self.j as usize] = val_i;
            let k = self.s[(val_i.wrapping_add(val_j)) as usize];
            *byte ^= k;
        }
    }
}

pub struct DiffieHellman {
    pub x: BigUint,
    pub y: BigUint,
}

impl Default for DiffieHellman {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffieHellman {
    pub fn new() -> Self {
        let p = get_dh_prime();
        let g = BigUint::from(2u32);
        let mut rng = rand::rng();
        let mut key_bytes = [0u8; 20];
        rng.fill_bytes(&mut key_bytes);
        let x = BigUint::from_bytes_be(&key_bytes);
        let y = g.modpow(&x, &p);
        Self { x, y }
    }

    pub fn compute_shared_secret(&self, other_y: &[u8]) -> crate::Result<Vec<u8>> {
        let p = get_dh_prime();
        let other_y_val = BigUint::from_bytes_be(other_y);
        if other_y_val == BigUint::from(0u32) || other_y_val >= p {
            return Err(crate::Error::Protocol("Invalid DH public key".to_string()));
        }
        let s = other_y_val.modpow(&self.x, &p);
        let mut s_bytes = s.to_bytes_be();
        if s_bytes.len() < 96 {
            let mut padded = vec![0u8; 96 - s_bytes.len()];
            padded.extend_from_slice(&s_bytes);
            s_bytes = padded;
        } else if s_bytes.len() > 96 {
            s_bytes = s_bytes[s_bytes.len() - 96..].to_vec();
        }
        Ok(s_bytes)
    }
}

fn get_dh_prime() -> BigUint {
    let prime_hex = "FFFFFFFFFFFFFFFFC90FDAA22168C234C4C6628B80DC1CD1\
                     29024E088A67CC74020BBEA63B139B22514A08798E3404DD\
                     EF9519B3CD3A431B302B0A6DF25F14374FE1356D6D51C245\
                     E485B576625E7EC6F44C42E9A63A3620FFFFFFFFFFFFFFFF";
    let bytes = hex::decode(prime_hex).unwrap();
    BigUint::from_bytes_be(&bytes)
}

pub fn hash_sha1(key: &[u8], s: &[u8], info_hash: Option<&[u8; 20]>) -> [u8; 20] {
    // codeql[rust/weak-cryptographic-algorithm]
    let mut hasher = Sha1::new();
    hasher.update(key);
    hasher.update(s);
    if let Some(ih) = info_hash {
        hasher.update(ih);
    }
    hasher.finalize().into()
}

#[derive(Debug)]
pub struct MseStream<S> {
    pub inner: S,
    pub encryptor: Option<Rc4>,
    pub decryptor: Option<Rc4>,
    write_buffer: Vec<u8>,
}

impl<S> MseStream<S> {
    pub fn new(inner: S) -> Self {
        Self {
            inner,
            encryptor: None,
            decryptor: None,
            write_buffer: Vec::new(),
        }
    }
}

impl<S> MseStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_flush_buffer(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        if this.write_buffer.is_empty() {
            return Poll::Ready(Ok(()));
        }
        let res = Pin::new(&mut this.inner).poll_write(cx, &this.write_buffer);
        match res {
            Poll::Ready(Ok(n)) => {
                if n > 0 {
                    this.write_buffer.drain(0..n);
                }
                if this.write_buffer.is_empty() {
                    Poll::Ready(Ok(()))
                } else {
                    Poll::Pending
                }
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<S> AsyncRead for MseStream<S>
where
    S: AsyncRead + Unpin,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before_len = buf.filled().len();
        let pin_inner = Pin::new(&mut self.inner);
        let res = pin_inner.poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &res {
            let after_len = buf.filled().len();
            if after_len > before_len {
                if let Some(ref mut rc4) = self.decryptor {
                    let filled = buf.filled_mut();
                    let new_data = &mut filled[before_len..after_len];
                    rc4.process(new_data);
                }
            }
        }
        res
    }
}

impl<S> AsyncWrite for MseStream<S>
where
    S: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.encryptor.is_none() {
            return Pin::new(&mut self.inner).poll_write(cx, buf);
        }
        if !self.write_buffer.is_empty() {
            match self.as_mut().poll_flush_buffer(cx) {
                Poll::Ready(Ok(())) => {}
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        let mut temp = buf.to_vec();
        if let Some(ref mut rc4) = self.encryptor {
            rc4.process(&mut temp);
        }
        self.write_buffer.extend_from_slice(&temp);
        match self.as_mut().poll_flush_buffer(cx) {
            Poll::Ready(Ok(())) => Poll::Ready(Ok(buf.len())),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Ready(Ok(buf.len())),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.as_mut().poll_flush_buffer(cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut self.inner).poll_flush(cx),
            other => other,
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.as_mut().poll_flush_buffer(cx) {
            Poll::Ready(Ok(())) => Pin::new(&mut self.inner).poll_shutdown(cx),
            other => other,
        }
    }
}
