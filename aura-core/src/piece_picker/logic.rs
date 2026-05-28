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

    /// Picks the next piece to download.
    /// If `sequential` is true, it picks the first available piece in order.
    /// Otherwise, it uses the rarest-first strategy.
    pub fn pick_next(
        &mut self,
        my_bitfield: &Bitfield,
        peer_addr: &str,
        sequential: bool,
    ) -> Option<usize> {
        if self.is_endgame(my_bitfield) {
            let res = self.pick_next_endgame(my_bitfield, peer_addr);
            if let Some(idx) = res {
                self.in_progress.set(idx, true);
            }
            return res;
        }

        let peer_bf = self.peer_bitfields.get(peer_addr)?;

        if sequential {
            for i in 0..self.num_pieces {
                if !my_bitfield.get(i) && peer_bf.get(i) && !self.in_progress.get(i) {
                    self.in_progress.set(i, true);
                    return Some(i);
                }
            }
            return None;
        }

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

        use rand::prelude::IndexedRandom;
        let res = rarest_pieces.choose(&mut rand::rng()).copied();
        if let Some(idx) = res {
            self.in_progress.set(idx, true);
        }
        res
    }

    /// Determines if the task is in "Endgame Mode".
    pub fn is_endgame(&self, my_bitfield: &Bitfield) -> bool {
        let remaining = self.num_pieces - my_bitfield.count_set();
        // Endgame triggers when very few pieces are left (e.g., < 3 or < 1%)
        if remaining == 0 {
            return false;
        }
        remaining <= 3 || (remaining as f32 / self.num_pieces as f32) < 0.01
    }

    /// Picks a piece even if it's already in progress (redundant fetching).
    pub fn pick_next_endgame(&self, my_bitfield: &Bitfield, peer_addr: &str) -> Option<usize> {
        let peer_bf = self.peer_bitfields.get(peer_addr)?;

        let mut available = Vec::new();
        for i in 0..self.num_pieces {
            if !my_bitfield.get(i) && peer_bf.get(i) {
                available.push(i);
            }
        }

        use rand::prelude::IndexedRandom;
        available.choose(&mut rand::rng()).copied()
    }
}

/// An RAII guard that automatically releases a picked piece back to the picker if dropped,
/// unless explicitly marked as completed.
pub struct PieceGuard {
    piece_idx: usize,
    on_drop: Option<Box<dyn FnOnce(usize) + Send + Sync>>,
}

impl std::fmt::Debug for PieceGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PieceGuard")
            .field("piece_idx", &self.piece_idx)
            .finish()
    }
}

impl PieceGuard {
    pub fn new<F>(piece_idx: usize, on_drop: F) -> Self
    where
        F: FnOnce(usize) + Send + Sync + 'static,
    {
        Self {
            piece_idx,
            on_drop: Some(Box::new(on_drop)),
        }
    }

    pub fn piece_idx(&self) -> usize {
        self.piece_idx
    }

    pub fn complete(&mut self) {
        self.on_drop = None;
    }
}

impl Drop for PieceGuard {
    fn drop(&mut self) {
        if let Some(on_drop) = self.on_drop.take() {
            on_drop(self.piece_idx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_guard_raii_release() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let released = Arc::new(AtomicBool::new(false));
        let released_clone = released.clone();

        {
            let _guard = PieceGuard::new(5, move |idx| {
                assert_eq!(idx, 5);
                released_clone.store(true, Ordering::SeqCst);
            });
            assert!(!released.load(Ordering::SeqCst));
        }

        assert!(released.load(Ordering::SeqCst));
    }

    #[test]
    fn test_piece_guard_complete_prevents_release() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let released = Arc::new(AtomicBool::new(false));
        let released_clone = released.clone();

        {
            let mut guard = PieceGuard::new(5, move |idx| {
                assert_eq!(idx, 5);
                released_clone.store(true, Ordering::SeqCst);
            });
            guard.complete();
        }

        assert!(!released.load(Ordering::SeqCst));
    }

    #[test]
    fn test_rarest_first_selection() {
        let mut picker = PiecePicker::new(10);
        let my_bf = Bitfield::new(10);

        // Peer A has pieces 0, 1, 2
        let mut bf_a = Bitfield::new(10);
        bf_a.set(0, true);
        bf_a.set(1, true);
        bf_a.set(2, true);
        picker.add_peer_bitfield("1.1.1.1:80".to_string(), bf_a);

        // Peer B has pieces 0, 3
        let mut bf_b = Bitfield::new(10);
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
            .pick_next(&my_bf, "1.1.1.1:80", false)
            .expect("Should pick a piece");
        assert!(picked == 1 || picked == 2);

        let expected_next = if picked == 1 { 2 } else { 1 };

        let mut my_bf_updated = my_bf.clone();
        my_bf_updated.set(picked, true);

        let picked2 = picker
            .pick_next(&my_bf_updated, "1.1.1.1:80", false)
            .expect("Should pick another piece");
        assert_eq!(picked2, expected_next);
    }

    #[test]
    fn test_endgame_mode_trigger() {
        let mut picker = PiecePicker::new(2);
        let my_bf = Bitfield::new(2);
        let mut peer_bf = Bitfield::new(2);
        peer_bf.set(0, true);
        peer_bf.set(1, true);
        picker.add_peer_bitfield("peer1".to_string(), peer_bf);

        // Mark all pieces as in progress
        picker.mark_in_progress(0);
        picker.mark_in_progress(1);

        // Standard pick SHOULD now return a piece because we are in endgame (2/2 pieces)
        let picked = picker.pick_next(&my_bf, "peer1", false);
        assert!(picked.is_some());

        // Also verify explicitly calling pick_next_endgame
        let picked_explicit = picker.pick_next_endgame(&my_bf, "peer1");
        assert!(picked_explicit.is_some());
    }
}
