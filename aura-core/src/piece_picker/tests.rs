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
        .pick_next(&my_bf, "1.1.1.1:80", false, false)
        .expect("Should pick a piece");
    assert!(picked == 1 || picked == 2);

    let expected_next = if picked == 1 { 2 } else { 1 };

    let mut my_bf_updated = my_bf.clone();
    my_bf_updated.set(picked, true);

    let picked2 = picker
        .pick_next(&my_bf_updated, "1.1.1.1:80", false, false)
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
    let picked = picker.pick_next(&my_bf, "peer1", false, false);
    assert!(picked.is_some());

    // Also verify explicitly calling pick_next_endgame
    let picked_explicit = picker.pick_next_endgame(&my_bf, "peer1");
    assert!(picked_explicit.is_some());
}

#[test]
fn test_streaming_mode_priority() {
    let mut picker = PiecePicker::new(10);
    picker.streaming_metadata_pieces = 3;
    let my_bf = Bitfield::new(10);

    // Peer has all pieces
    let mut peer_bf = Bitfield::new(10);
    for i in 0..10 {
        peer_bf.set(i, true);
    }
    picker.add_peer_bitfield("1.1.1.1:80".to_string(), peer_bf);

    // Pick 1: Should be piece 0 (beginning)
    let picked1 = picker.pick_next(&my_bf, "1.1.1.1:80", false, true).unwrap();
    assert_eq!(picked1, 0);

    // Pick 2: Should be piece 1 (beginning)
    let mut my_bf_updated = my_bf.clone();
    my_bf_updated.set(0, true);
    let picked2 = picker
        .pick_next(&my_bf_updated, "1.1.1.1:80", false, true)
        .unwrap();
    assert_eq!(picked2, 1);

    // Pick 3: Should be piece 2 (beginning)
    my_bf_updated.set(1, true);
    let picked3 = picker
        .pick_next(&my_bf_updated, "1.1.1.1:80", false, true)
        .unwrap();
    assert_eq!(picked3, 2);

    // Pick 4: Should be piece 7 (end: 10 - 3 = 7)
    my_bf_updated.set(2, true);
    let picked4 = picker
        .pick_next(&my_bf_updated, "1.1.1.1:80", false, true)
        .unwrap();
    assert_eq!(picked4, 7);

    // Pick 5: Should be piece 8 (end)
    my_bf_updated.set(7, true);
    let picked5 = picker
        .pick_next(&my_bf_updated, "1.1.1.1:80", false, true)
        .unwrap();
    assert_eq!(picked5, 8);

    // Pick 6: Should be piece 9 (end)
    my_bf_updated.set(8, true);
    let picked6 = picker
        .pick_next(&my_bf_updated, "1.1.1.1:80", false, true)
        .unwrap();
    assert_eq!(picked6, 9);

    // Pick 7: Should fall back to rarest-first or random/first available (since 0-2 and 7-9 are done)
    // The remaining pieces are 3, 4, 5, 6
    my_bf_updated.set(9, true);
    let picked7 = picker
        .pick_next(&my_bf_updated, "1.1.1.1:80", false, true)
        .unwrap();
    assert!((3..=6).contains(&picked7));
}
