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
