use sha2::{Digest, Sha256};

impl super::super::logic::Torrent {
    /// Computes the SHA-256 Merkle root of a piece according to BEP 52.
    /// The piece is divided into 16KiB blocks. The block hashes are the leaves.
    /// The tree is padded with 32-byte zero hashes to a power of two.
    pub fn compute_piece_merkle_root(data: &[u8]) -> [u8; 32] {
        const BLOCK_SIZE: usize = 16384;
        if data.is_empty() {
            return [0; 32];
        }

        let mut leaves: Vec<[u8; 32]> = data
            .chunks(BLOCK_SIZE)
            .map(|chunk| {
                let mut hasher = Sha256::new();
                hasher.update(chunk);
                hasher.finalize().into()
            })
            .collect();

        if leaves.is_empty() {
            return [0; 32];
        }

        // Pad to next power of two with zero hashes
        let next_pow2 = leaves.len().next_power_of_two();
        leaves.resize(next_pow2, [0; 32]);

        let mut current_level = leaves;

        while current_level.len() > 1 {
            let mut next_level = Vec::with_capacity(current_level.len() / 2);
            for pair in current_level.chunks(2) {
                let mut hasher = Sha256::new();
                hasher.update(pair[0]);
                hasher.update(pair[1]);
                next_level.push(hasher.finalize().into());
            }
            current_level = next_level;
        }

        current_level[0]
    }
}
