use super::*;
use crate::peer_registry::ConnectionState;
use crate::tracker::Peer;
use std::time::{Duration, Instant};

fn make_test_peer_state(
    download_rate: f64,
    upload_rate: f64,
    error_count: u32,
    idle_duration: Duration,
    am_interested: bool,
    peer_choking: bool,
) -> PeerState {
    PeerState {
        peer: Peer {
            ip: "127.0.0.1".to_string(),
            port: 6881,
            id: None,
        },
        state: ConnectionState::Handshaked,
        am_choking: false,
        am_interested,
        peer_choking,
        peer_interested: false,
        downloaded_bytes: 0,
        last_downloaded_bytes: 0,
        download_rate,
        uploaded_bytes: 0,
        last_uploaded_bytes: 0,
        upload_rate,
        is_optimistic_unchoke: false,
        last_activity: Instant::now() - idle_duration,
        error_count,
    }
}

#[test]
fn test_throughput_priority_scorer() {
    let scorer = ThroughputPriorityScorer;

    // High download rate peer
    let fast_peer = make_test_peer_state(100.0, 10.0, 0, Duration::from_secs(5), false, false);
    // High upload rate peer
    let seed_peer = make_test_peer_state(10.0, 100.0, 0, Duration::from_secs(5), false, false);

    let score_fast = scorer.calculate_score(&fast_peer);
    let score_seed = scorer.calculate_score(&seed_peer);

    // ThroughputPriorityScorer weights download_rate double
    assert!(
        score_fast > score_seed,
        "Fast download peer should score higher than seed peer"
    );
}

#[test]
fn test_anti_snubbing_scorer() {
    let scorer = AntiSnubbingScorer {
        snub_timeout_secs: 10.0,
    };

    // We are interested, but they haven't sent anything in 15 seconds (snubbed)
    let snubbed_peer = make_test_peer_state(0.0, 0.0, 0, Duration::from_secs(15), true, false);

    // We are interested, they haven't sent anything in 5 seconds (not yet snubbed)
    let fine_peer = make_test_peer_state(0.0, 0.0, 0, Duration::from_secs(5), true, false);

    let score_snubbed = scorer.calculate_score(&snubbed_peer);
    let score_fine = scorer.calculate_score(&fine_peer);

    assert!(
        score_snubbed < score_fine,
        "Snubbed peer should be heavily penalized"
    );

    // We are interested, and they are choking us
    let choked_peer = make_test_peer_state(0.0, 0.0, 0, Duration::from_secs(5), true, true);
    let score_choked = scorer.calculate_score(&choked_peer);
    assert!(score_choked < score_fine, "Choked peer should be penalized");
}
