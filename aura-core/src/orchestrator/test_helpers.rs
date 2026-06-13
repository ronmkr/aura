use crate::net_util::TokioResolver;
use crate::orchestrator::Orchestrator;
use arc_swap::ArcSwap;
use std::sync::Arc;
use tokio::sync::mpsc;

pub mod mock_storage;
pub use mock_storage::MockStorage;

pub fn create_test_orchestrator() -> (Orchestrator, mpsc::Receiver<crate::storage::StorageRequest>)
{
    let (_command_tx, command_rx) = mpsc::channel(1024);
    let (storage_tx, storage_rx) = mpsc::channel(1024);
    let (_storage_event_tx, storage_event_rx) = mpsc::channel(1024);
    let (_event_tx, _event_rx) =
        tokio::sync::broadcast::channel::<crate::orchestrator::Event>(1024);

    let (dht_tx, _dht_rx) = mpsc::channel(1024);
    let (lpd_tx, _lpd_rx) = mpsc::channel(1024);
    let (_scrub_tx, _scrub_rx) = mpsc::channel::<crate::scrubber::ScrubberCommand>(1024);

    let config = Arc::new(ArcSwap::from_pointee(crate::Config::default()));

    let (orch, _tx) = Orchestrator::new(
        crate::orchestrator::state::OrchestratorChannels {
            command_rx,
            storage_client: Arc::new(crate::storage::StorageClient::new(storage_tx)),
            storage_completion_rx: storage_event_rx,
            dht_tx,
            lpd_tx,
            nat_tx: mpsc::channel(1).0,
        },
        config,
        sled::Config::new().temporary(true).open().unwrap(),
        Arc::new(TokioResolver::builder_tokio().unwrap().build().unwrap()),
    );

    (orch, storage_rx)
}
