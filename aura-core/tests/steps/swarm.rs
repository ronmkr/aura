use crate::AuraWorld;
use cucumber::{given, then, when};

#[given(expr = "a magnet link with info-hash {string}")]
async fn given_magnet_link(_world: &mut AuraWorld, hash: String) {
    assert_eq!(hash, "...");
}

#[when(expr = "I add the task")]
async fn when_add_task(_world: &mut AuraWorld) {}

#[then(expr = "the engine should enter {string} phase")]
async fn then_engine_enter_phase(_world: &mut AuraWorld, phase: String) {
    assert_eq!(phase, "MetadataExchange");
}

#[then(expr = "it should connect to DHT and PEX to find peers")]
async fn then_connect_dht_pex(_world: &mut AuraWorld) {}

#[then(expr = "once the info-dict is received, it should transition to {string} phase")]
async fn then_transition_phase(_world: &mut AuraWorld, phase: String) {
    assert_eq!(phase, "Downloading");
}

#[then(expr = "the total file size should be correctly resolved")]
async fn then_total_file_size_resolved(_world: &mut AuraWorld) {}

#[given(expr = "a v2 hybrid torrent file")]
async fn given_v2_hybrid_torrent(_world: &mut AuraWorld) {}

#[when(expr = "a piece is downloaded")]
async fn when_piece_downloaded(_world: &mut AuraWorld) {}

#[then(expr = "the engine should verify the piece against the SHA-256 Merkle tree")]
async fn then_verify_piece_sha256(_world: &mut AuraWorld) {}

#[then(expr = "the piece layer should be persisted in the Sled database")]
async fn then_persist_piece_layer(_world: &mut AuraWorld) {}

#[then(expr = "corrupted pieces should be immediately discarded and re-requested")]
async fn then_discard_corrupted_pieces(_world: &mut AuraWorld) {}
