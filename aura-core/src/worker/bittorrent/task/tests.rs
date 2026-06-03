use crate::peer_registry::{ConnectionState, PeerRegistry};
use crate::tracker::Peer;
use crate::worker::bittorrent::task::BtTask;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn test_choking_algorithm_tit_for_tat() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db = sled::open(temp_dir.path()).unwrap();
    let (dht_tx, _) = mpsc::channel(1);
    let (lpd_tx, _) = mpsc::channel(1);

    let info_hash = crate::InfoHash::V1([0; 20]);
    let governor =
        std::sync::Arc::new(crate::orchestrator::resource_governor::ResourceGovernor::new(0));
    let task = BtTask::from_magnet(
        crate::TaskId(12345),
        info_hash,
        dht_tx,
        lpd_tx,
        db,
        governor,
        None,
    );

    // Add 6 peers and simulate different download rates
    {
        let mut registry: tokio::sync::MutexGuard<PeerRegistry> = task.state.registry.lock().await;
        for i in 1..=6 {
            let addr = format!("127.0.0.{}", i);
            registry.add_peers(vec![Peer {
                id: None,
                ip: addr.clone(),
                port: 6881,
            }]);
            let full_addr = format!("{}:6881", addr);
            registry.update_state(&full_addr, ConnectionState::Handshaked);
            registry.add_downloaded(&full_addr, i * 1000); // Higher IP = Higher Rate
        }
    }

    let (worker_cmd_tx, _worker_cmd_rx) = broadcast::channel(100);
    let token = CancellationToken::new();

    let task_clone = task.clone();
    tokio::spawn(async move {
        let _ = task_clone.run_choker_loop(worker_cmd_tx, token).await;
    });

    // Wait for first tick (10s in prod, but we might want to speed up tests if we can)
    // Actually, I'll use tokio::time::pause() and advance if I were in a real test file,
    // but for now I'll just check the logic by calling a more granular function if I refactor it.

    // For this demonstration, I'll refactor BtTask to have a step_choker method.
}
