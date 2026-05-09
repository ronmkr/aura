use crate::AuraWorld;
use cucumber::{given, then, when};

#[given(expr = "a new download task for {string}")]
async fn given_new_download_task(_world: &mut AuraWorld, _file: String) {}

#[when(expr = "the download is in progress")]
async fn when_download_in_progress(_world: &mut AuraWorld) {}

#[then(expr = "a file named {string} should exist in the download directory")]
async fn then_part_file_exists(_world: &mut AuraWorld, _file: String) {}

#[then(expr = "{string} should NOT exist")]
async fn then_file_not_exists(_world: &mut AuraWorld, _file: String) {}

#[when(expr = "the download reaches {int}% and integrity is verified")]
async fn when_download_reaches_and_verified(_world: &mut AuraWorld, _percent: u32) {}

#[then(expr = "{string} should be renamed to {string}")]
async fn then_file_renamed(_world: &mut AuraWorld, _old: String, _new: String) {}

#[then(expr = "the .aura control file should be deleted")]
async fn then_control_file_deleted(_world: &mut AuraWorld) {}

#[given(expr = "a BitTorrent swarm delivering pieces out of order")]
async fn given_bittorrent_swarm_out_of_order(_world: &mut AuraWorld) {}

#[when(expr = "Piece {int} \\({int}MB-{int}MB) arrives before Piece {int} \\({int}MB-{int}MB)")]
async fn when_pieces_arrive_out_of_order(
    _world: &mut AuraWorld,
    _p5: u32,
    _p5_start: u32,
    _p5_end: u32,
    _p4: u32,
    _p4_start: u32,
    _p4_end: u32,
) {
}

#[then(expr = "the Storage Engine should buffer Piece {int} in memory")]
async fn then_storage_engine_buffers_piece(_world: &mut AuraWorld, _piece: u32) {}

#[when(expr = "Piece {int} arrives and is written")]
async fn when_piece_arrives_and_written(_world: &mut AuraWorld, _piece: u32) {}

#[then(expr = "the Storage Engine should immediately flush Piece {int} to disk")]
async fn then_storage_engine_flushes_piece(_world: &mut AuraWorld, _piece: u32) {}

#[then(expr = "the disk seek count should be minimized")]
async fn then_disk_seek_count_minimized(_world: &mut AuraWorld) {}
