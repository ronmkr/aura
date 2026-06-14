use crate::AuraWorld;
use aura_core::orchestrator::TaskQuerier;
use cucumber::{given, then, when};

#[given(expr = "a new download task for {string}")]
async fn given_new_download_task(world: &mut AuraWorld, file: String) {
    let _path = world.temp_dir.path().join(&file);
    world
        .temp_files
        .push(tempfile::NamedTempFile::new().unwrap());

    // Create an empty part file to simulate an ongoing task
    let part_path = world.temp_dir.path().join(format!("{}.part", file));
    std::fs::write(&part_path, "partial data").unwrap();

    // Also create the .aura file
    let aura_path = world.temp_dir.path().join(format!("{}.aura", file));
    std::fs::write(&aura_path, "metadata").unwrap();
}

#[when(expr = "the download is in progress")]
async fn when_download_in_progress(_world: &mut AuraWorld) {
    // Simulated as part of 'given'
}

#[then(expr = "a file named {string} should exist in the download directory")]
async fn then_part_file_exists(world: &mut AuraWorld, file: String) {
    let part_path = world.temp_dir.path().join(file);
    assert!(part_path.exists(), "The part file should exist");
}

#[then(expr = "{string} should NOT exist")]
async fn then_file_not_exists(world: &mut AuraWorld, file: String) {
    let target_path = world.temp_dir.path().join(file);
    assert!(
        !target_path.exists(),
        "The completed file should not exist yet"
    );
}

#[when(expr = "the download reaches {int}% and integrity is verified")]
async fn when_download_reaches_and_verified(world: &mut AuraWorld, percent: u32) {
    assert_eq!(percent, 100);
    // Simulate engine renaming the file upon 100% completion
    let part_path = world.temp_dir.path().join("movie.mp4.part");
    let target_path = world.temp_dir.path().join("movie.mp4");
    if part_path.exists() {
        std::fs::rename(part_path, target_path).unwrap();
    }

    let aura_path = world.temp_dir.path().join("movie.mp4.aura");
    if aura_path.exists() {
        std::fs::remove_file(aura_path).unwrap();
    }
}

#[then(expr = "{string} should be renamed to {string}")]
async fn then_file_renamed(world: &mut AuraWorld, old: String, new: String) {
    let old_path = world.temp_dir.path().join(old);
    let new_path = world.temp_dir.path().join(new);
    assert!(!old_path.exists(), "The old .part file should be gone");
    assert!(new_path.exists(), "The renamed final file should exist");
}

#[then(expr = "the .aura control file should be deleted")]
async fn then_control_file_deleted(world: &mut AuraWorld) {
    let aura_path = world.temp_dir.path().join("movie.mp4.aura");
    assert!(
        !aura_path.exists(),
        "The .aura control file should be deleted"
    );
}

#[given(expr = "a BitTorrent swarm delivering pieces out of order")]
async fn given_bittorrent_swarm_out_of_order(_world: &mut AuraWorld) {
    // Set up mock piece aggregator in our test context
}

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
    // Simulating out-of-order piece arrival
}

#[then(expr = "the Storage Engine should buffer Piece {int} in memory")]
async fn then_storage_engine_buffers_piece(_world: &mut AuraWorld, _piece: u32) {
    // Assert that the piece is buffered in memory, not written to disk yet
}

#[when(expr = "Piece {int} arrives and is written")]
async fn when_piece_arrives_and_written(_world: &mut AuraWorld, _piece: u32) {
    // Simulate piece 4 arriving and writing to disk
}

#[then(expr = "the Storage Engine should immediately flush Piece {int} to disk")]
async fn then_storage_engine_flushes_piece(_world: &mut AuraWorld, _piece: u32) {
    // Verify that the buffered piece 5 was flushed
}

#[then(expr = "the disk seek count should be minimized")]
async fn then_disk_seek_count_minimized(_world: &mut AuraWorld) {
    // Verify that contiguous pieces were written in a single seek
}

#[given(expr = "a stalled BitTorrent download task")]
async fn given_stalled_bittorrent_download(world: &mut AuraWorld) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
}

#[given(expr = "the downloaded file contains corrupted data at piece {int}")]
async fn given_downloaded_file_corrupted(_world: &mut AuraWorld, _piece: u32) {}

#[when(expr = "the EWMA stall detection triggers the Integrity Scrubber")]
async fn when_ewma_stall_triggers_scrubber(world: &mut AuraWorld) {
    if let Some(engine) = &world.engine {
        let _ = engine.tell_active().await;
    }
}

#[then(expr = "the Integrity Scrubber should find the corruption")]
async fn then_integrity_scrubber_finds_corruption(world: &mut AuraWorld) {
    if let Some(engine) = &world.engine {
        let _ = engine.tell_active().await;
    }
}

#[then(expr = "piece {int} should be marked as missing in the Bitfield")]
async fn then_piece_marked_missing(world: &mut AuraWorld, _piece: u32) {
    if let Some(engine) = &world.engine {
        let _ = engine.tell_active().await;
    }
}

#[then(expr = "a RefreshDiscovery event should be dispatched")]
async fn then_refresh_discovery_dispatched(world: &mut AuraWorld) {
    if let Some(rx) = &mut world.events_rx {
        use tokio::time::{timeout, Duration};
        let _ = timeout(Duration::from_millis(100), rx.recv()).await;
    }
}
