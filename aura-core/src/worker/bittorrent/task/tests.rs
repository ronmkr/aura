use super::*;
use crate::InfoHash;
use std::sync::Arc;
use tokio::sync::mpsc;

#[tokio::test]
async fn test_bt_task_from_magnet() {
    let db = sled::Config::new().temporary(true).open().unwrap();
    let governor = Arc::new(
        crate::orchestrator::resource_governor::ResourceGovernor::new(
            100 * 1024 * 1024,
            50 * 1024 * 1024,
        ),
    );
    let info_hash = InfoHash::V1([0; 20]);
    let (dht_tx, _) = mpsc::channel(1);
    let (lpd_tx, _) = mpsc::channel(1);

    let task = BtTask::from_magnet(BtTaskFromMagnetArgs {
        id: crate::TaskId(12345),
        info_hash,
        trackers: Vec::new(),
        dht_tx,
        lpd_tx,
        db,
        resource_governor: governor,
        tenant_id: None,
        config: std::sync::Arc::new(arc_swap::ArcSwap::new(std::sync::Arc::new(
            crate::Config::default(),
        ))),
        streaming_mode: false,
    });

    assert_eq!(task.id, crate::TaskId(12345));
}
