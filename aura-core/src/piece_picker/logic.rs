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
    /// The pieces that are selected for download.
    pub selected_pieces: Bitfield,
    /// Configuration for endgame mode
    pub endgame_threshold_pieces: usize,
    pub endgame_threshold_percent: f32,
}

impl PiecePicker {
    /// Creates a new PiecePicker for a task with a given number of pieces.
    pub fn new(num_pieces: usize) -> Self {
        let mut selected = Bitfield::new(num_pieces);
        for i in 0..num_pieces {
            selected.set(i, true);
        }
        Self {
            num_pieces,
            piece_counts: vec![0; num_pieces],
            peer_bitfields: HashMap::new(),
            in_progress: Bitfield::new(num_pieces),
            selected_pieces: selected,
            endgame_threshold_pieces: 3,
            endgame_threshold_percent: 0.01,
        }
    }

    /// Creates a new PiecePicker with specific piece selection.
    pub fn with_selection(
        num_pieces: usize,
        selected_pieces: Bitfield,
        endgame_threshold_pieces: usize,
        endgame_threshold_percent: f32,
    ) -> Self {
        Self {
            num_pieces,
            piece_counts: vec![0; num_pieces],
            peer_bitfields: HashMap::new(),
            in_progress: Bitfield::new(num_pieces),
            selected_pieces,
            endgame_threshold_pieces,
            endgame_threshold_percent,
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
        if let Some(target) = self.peer_bitfields.get_mut(&addr) {
            // Merge into existing: only update counts for pieces the peer didn't have before
            for i in 0..self.num_pieces {
                let has = bitfield.get(i);
                if has && !target.get(i) {
                    self.piece_counts[i] += 1;
                    target.set(i, true);
                }
            }
        } else {
            // New peer: add all counts
            for i in 0..self.num_pieces {
                if bitfield.get(i) {
                    self.piece_counts[i] += 1;
                }
            }
            self.peer_bitfields.insert(addr, bitfield);
        }
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
        let peer_bf = match self.peer_bitfields.get(peer_addr) {
            Some(bf) => bf,
            None => {
                tracing::debug!(addr = %peer_addr, "Peer bitfield not found in picker");
                return None;
            }
        };

        if self.is_endgame(my_bitfield) {
            let res = self.pick_next_endgame(my_bitfield, peer_addr);
            if let Some(idx) = res {
                self.in_progress.set(idx, true);
            }
            return res;
        }

        if sequential {
            for i in 0..self.num_pieces {
                if self.selected_pieces.get(i)
                    && !my_bitfield.get(i)
                    && peer_bf.get(i)
                    && !self.in_progress.get(i)
                {
                    self.in_progress.set(i, true);
                    return Some(i);
                }
            }
            return None;
        }

        let mut rarest_pieces = Vec::new();
        let mut min_count = usize::MAX;
        let mut total_candidates = 0;

        for i in 0..self.num_pieces {
            if !self.selected_pieces.get(i)
                || my_bitfield.get(i)
                || !peer_bf.get(i)
                || self.in_progress.get(i)
            {
                continue;
            }

            total_candidates += 1;
            let count = self.piece_counts[i];
            if count < min_count {
                min_count = count;
                rarest_pieces.clear();
                rarest_pieces.push(i);
            } else if count == min_count {
                rarest_pieces.push(i);
            }
        }

        if total_candidates == 0 {
            tracing::info!(
                addr = %peer_addr,
                my_count = my_bitfield.count_set(),
                num_pieces = self.num_pieces,
                "No piece candidates found for peer (already have them or in-progress)"
            );
        } else {
            tracing::info!(
                addr = %peer_addr,
                candidates = total_candidates,
                "Found piece candidates for peer"
            );
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
        let mut selected_count = 0;
        let mut my_selected_count = 0;
        for i in 0..self.num_pieces {
            if self.selected_pieces.get(i) {
                selected_count += 1;
                if my_bitfield.get(i) {
                    my_selected_count += 1;
                }
            }
        }
        let remaining = selected_count - my_selected_count;
        if remaining == 0 {
            return false;
        }
        remaining <= 3 || (remaining as f32 / selected_count as f32) < 0.01
    }

    /// Picks a piece even if it's already in progress (redundant fetching).
    pub fn pick_next_endgame(&self, my_bitfield: &Bitfield, peer_addr: &str) -> Option<usize> {
        let peer_bf = self.peer_bitfields.get(peer_addr)?;

        let mut available = Vec::new();
        for i in 0..self.num_pieces {
            if self.selected_pieces.get(i) && !my_bitfield.get(i) && peer_bf.get(i) {
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
#[path = "tests.rs"]
mod tests;
