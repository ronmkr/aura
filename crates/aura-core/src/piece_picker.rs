//! piece_picker: Implements the rarest-first piece selection strategy.

use crate::bitfield::Bitfield;
use std::collections::HashMap;

pub type PeerId = [u8; 20];

/// Manages piece selection logic, prioritizing the rarest pieces in the swarm.
#[derive(Debug)]
pub struct PiecePicker {
    pub num_pieces: usize,
    /// Number of peers that have each piece.
    piece_counts: Vec<usize>,
    /// Tracks which pieces each peer has.
    peer_bitfields: HashMap<String, Bitfield>,
    /// Pieces currently being requested by workers.
    in_progress: Bitfield,
}

impl PiecePicker {
    /// Creates a new PiecePicker for a task with a given number of pieces.
    pub fn new(num_pieces: usize) -> Self {
        Self {
            num_pieces,
            piece_counts: vec![0; num_pieces],
            peer_bitfields: HashMap::new(),
            in_progress: Bitfield::new(num_pieces),
        }
    }

    pub fn mark_in_progress(&mut self, piece_idx: usize) {
        self.in_progress.set(piece_idx, true);
    }

    pub fn mark_completed(&mut self, piece_idx: usize) {
        self.in_progress.set(piece_idx, false);
    }

    pub fn release_piece(&mut self, piece_idx: usize) {
        self.in_progress.set(piece_idx, false);
    }

    /// Records the bitfield of a new peer or updates an existing one.
    pub fn add_peer_bitfield(&mut self, addr: String, bitfield: Bitfield) {
        // If we already had this peer, subtract its old bitfield counts
        if let Some(old_bf) = self.peer_bitfields.get(&addr) {
            for i in 0..self.num_pieces {
                if old_bf.get(i) {
                    self.piece_counts[i] -= 1;
                }
            }
        }

        // Add new bitfield counts
        for i in 0..self.num_pieces {
            if bitfield.get(i) {
                self.piece_counts[i] += 1;
            }
        }

        self.peer_bitfields.insert(addr, bitfield);
    }

    /// Removes a peer and its contribution to piece counts.
    pub fn remove_peer(&mut self, addr: &str) {
        if let Some(bf) = self.peer_bitfields.remove(addr) {
            for i in 0..self.num_pieces {
                if bf.get(i) {
                    self.piece_counts[i] -= 1;
                }
            }
        }
    }

    /// Picks the next piece to download using the rarest-first strategy,
    /// considering only pieces that the specified peer has.
    pub fn pick_next(&self, my_bitfield: &Bitfield, peer_addr: &str) -> Option<usize> {
        let peer_bf = self.peer_bitfields.get(peer_addr)?;

        let mut rarest_pieces = Vec::new();
        let mut min_count = usize::MAX;

        for i in 0..self.num_pieces {
            // Skip pieces I already have, or peer doesn't have, or already in progress
            if my_bitfield.get(i) || !peer_bf.get(i) || self.in_progress.get(i) {
                continue;
            }

            let count = self.piece_counts[i];
            if count < min_count {
                min_count = count;
                rarest_pieces.clear();
                rarest_pieces.push(i);
            } else if count == min_count {
                rarest_pieces.push(i);
            }
        }

        // Randomize from the rarest pieces to avoid thundering herds
        if rarest_pieces.is_empty() {
            None
        } else {
            use rand::seq::SliceRandom;
            rarest_pieces.choose(&mut rand::thread_rng()).copied()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rarest_first_selection() {
        let mut picker = PiecePicker::new(5);
        let my_bf = Bitfield::new(5);

        // Peer A has pieces 0, 1, 2
        let mut bf_a = Bitfield::new(5);
        bf_a.set(0, true);
        bf_a.set(1, true);
        bf_a.set(2, true);
        picker.add_peer_bitfield("1.1.1.1:80".to_string(), bf_a);

        // Peer B has pieces 0, 3
        let mut bf_b = Bitfield::new(5);
        bf_b.set(0, true);
        bf_b.set(3, true);
        picker.add_peer_bitfield("2.2.2.2:80".to_string(), bf_b);

        // Pieces availability:
        // 0: 2 peers
        // 1: 1 peer (Rare)
        // 2: 1 peer (Rare)
        // 3: 1 peer (Rare)
        // 4: 0 peers

        // Pick next piece. Since 1, 2, 3 are equally rare (1 peer),
        // but for Peer A only 1 or 2 are available.
        let picked = picker
            .pick_next(&my_bf, "1.1.1.1:80")
            .expect("Should pick a piece");
        assert!(picked == 1 || picked == 2);

        // If I now have piece 1, it should pick 2 for Peer A
        let mut my_bf_updated = my_bf.clone();
        my_bf_updated.set(1, true);
        let picked2 = picker
            .pick_next(&my_bf_updated, "1.1.1.1:80")
            .expect("Should pick another piece");
        assert_eq!(picked2, 2);
    }
}
