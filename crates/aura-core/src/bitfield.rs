//! bitfield: Tracks piece availability in the swarm.

use serde::{Deserialize, Serialize};

/// A compact representation of piece availability.
/// Uses a byte array where each bit represents one piece.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bitfield {
    bits: Vec<u8>,
    num_pieces: usize,
}

impl Bitfield {
    /// Creates a new Bitfield initialized to all zeros.
    pub fn new(num_pieces: usize) -> Self {
        let num_bytes = num_pieces.div_ceil(8);
        Self {
            bits: vec![0u8; num_bytes],
            num_pieces,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Sets the value of a bit at the given index.
    pub fn set(&mut self, index: usize, value: bool) {
        if index >= self.num_pieces {
            return;
        }
        let byte_idx = index / 8;
        let bit_idx = 7 - (index % 8); // BitTorrent uses big-endian bit order
        if value {
            self.bits[byte_idx] |= 1 << bit_idx;
        } else {
            self.bits[byte_idx] &= !(1 << bit_idx);
        }
    }

    /// Gets the value of a bit at the given index.
    pub fn get(&self, index: usize) -> bool {
        if index >= self.num_pieces {
            return false;
        }
        let byte_idx = index / 8;
        let bit_idx = 7 - (index % 8);
        (self.bits[byte_idx] >> bit_idx) & 1 == 1
    }

    /// Returns the number of pieces tracked by this bitfield.
    pub fn len(&self) -> usize {
        self.num_pieces
    }

    /// Returns the number of set bits (pieces available).
    pub fn count_set(&self) -> usize {
        let mut count = 0;
        for i in 0..self.num_pieces {
            if self.get(i) {
                count += 1;
            }
        }
        count
    }

    /// Returns true if all pieces are available.
    pub fn is_complete(&self) -> bool {
        self.count_set() == self.num_pieces
    }

    /// Returns the underlying byte representation.
    pub fn as_bytes(&self) -> Vec<u8> {
        self.bits.clone()
    }

    /// Creates a Bitfield from a byte array.
    pub fn from_bytes(bytes: &[u8], num_pieces: usize) -> Self {
        let mut bf = Self::new(num_pieces);
        let num_bytes = std::cmp::min(bytes.len(), bf.bits.len());
        bf.bits[..num_bytes].copy_from_slice(&bytes[..num_bytes]);
        bf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitfield_creation() {
        let bf = Bitfield::new(10);
        assert_eq!(bf.len(), 10);
        assert_eq!(bf.count_set(), 0);
        assert!(!bf.get(0));
    }

    #[test]
    fn test_bitfield_set_get() {
        let mut bf = Bitfield::new(10);
        bf.set(5, true);
        assert!(bf.get(5));
        assert_eq!(bf.count_set(), 1);
        bf.set(5, false);
        assert!(!bf.get(5));
        assert_eq!(bf.count_set(), 0);
    }

    #[test]
    fn test_bitfield_is_complete() {
        let mut bf = Bitfield::new(3);
        assert!(!bf.is_complete());
        bf.set(0, true);
        bf.set(1, true);
        bf.set(2, true);
        assert!(bf.is_complete());
    }

    #[test]
    fn test_bitfield_serialization() {
        let mut bf = Bitfield::new(10);
        bf.set(0, true);
        bf.set(7, true);
        bf.set(8, true);
        
        let bytes = bf.as_bytes();
        // 10 pieces = 2 bytes (8 + 2 bits)
        assert_eq!(bytes.len(), 2);
        // Byte 0: 10000001 (129)
        assert_eq!(bytes[0], 0b10000001);
        // Byte 1: 10000000 (128)
        assert_eq!(bytes[1], 0b10000000);
        
        let bf2 = Bitfield::from_bytes(&bytes, 10);
        assert!(bf2.get(0));
        assert!(bf2.get(7));
        assert!(bf2.get(8));
        assert!(!bf2.get(1));
    }
}
