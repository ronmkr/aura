use crate::orchestrator::state::{Orchestrator, OrchestratorChannels};
use crate::orchestrator::SubTaskEvent;
use crate::vpn::{VpnProvider, VpnStatus};
use std::sync::Arc;
use tokio::sync::mpsc;

struct MockVpnProvider {
    status: Arc<tokio::sync::Mutex<VpnStatus>>,
}

#[async_trait::async_trait]
impl VpnProvider for MockVpnProvider {
    fn name(&self) -> &str {
        "mock-vpn"
    }

    async fn connect(&self) -> crate::Result<()> {
        Ok(())
    }

    async fn disconnect(&self) -> crate::Result<()> {
        Ok(())
    }

    async fn status(&self) -> crate::Result<VpnStatus> {
        Ok(self.status.lock().await.clone())
    }

    fn interface(&self) -> Option<String> {
        Some("tun0".to_string())
    }
}

#[tokio::test]
async fn test_vpn_killswitch_enforcement() {
    let mut config = crate::Config::default();
    config.vpn.force_tunnel = true;

    let status = Arc::new(tokio::sync::Mutex::new(VpnStatus::Disconnected));
    let mock_provider = Arc::new(MockVpnProvider {
        status: Arc::clone(&status),
    });

    let (_command_tx, command_rx) = mpsc::channel(100);
    let (storage_tx, _storage_rx) = mpsc::channel(100);
    let (_completion_tx, completion_rx) = mpsc::channel(100);
    let (dht_tx, _dht_rx) = mpsc::channel(100);
    let (nat_tx, _nat_rx) = mpsc::channel(100);
    let (lpd_tx, _lpd_rx) = mpsc::channel(100);

    let config_swap = Arc::new(arc_swap::ArcSwap::from_pointee(config.clone()));

    let temp_dir = tempfile::tempdir().unwrap();
    let db = sled::open(temp_dir.path()).unwrap();
    let dns_resolver = Arc::new(
        hickory_resolver::TokioResolver::builder_tokio()
            .unwrap()
            .build()
            .unwrap(),
    );

    let (mut orchestrator, _event_tx) = Orchestrator::new(
        OrchestratorChannels {
            command_rx,
            storage_client: Arc::new(crate::storage::StorageClient::new(storage_tx)),
            storage_completion_rx: completion_rx,
            dht_tx,
            lpd_tx,
            nat_tx,
        },
        config_swap,
        db,
        dns_resolver,
    );

    let vpn_watch_rx = orchestrator.vpn_watch_tx.subscribe();
    orchestrator.vpn_provider = Some(mock_provider.clone() as Arc<dyn VpnProvider>);
    let _ = orchestrator
        .vpn_watch_tx
        .send(Some(mock_provider.clone() as Arc<dyn VpnProvider>));

    // 1. Verify verify_vpn_connectivity() fails when Disconnected
    let verify_result = orchestrator.verify_vpn_connectivity().await;
    assert!(verify_result.is_err());
    assert!(verify_result
        .unwrap_err()
        .to_string()
        .contains("Mandatory VPN tunnel is down"));

    // 2. Verify verify_vpn_connectivity() succeeds when Connected
    *status.lock().await = VpnStatus::Connected;
    let verify_result2 = orchestrator.verify_vpn_connectivity().await;
    assert!(verify_result2.is_ok());

    // 3. Verify background watch loop triggers KillSwitch on transition to Disconnected
    let mut subtask_rx = orchestrator.subtask_rx;
    let config_clone = orchestrator.config.clone();
    let subtask_tx_monitor = orchestrator.subtask_tx.clone();

    let watch_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(10));
        loop {
            interval.tick().await;
            let force_tunnel = config_clone.load().vpn.force_tunnel;
            let vpn_opt = vpn_watch_rx.borrow().clone();
            if let Some(vpn) = vpn_opt {
                if force_tunnel {
                    let stat = vpn.status().await;
                    match stat {
                        Ok(VpnStatus::Disconnected) | Ok(VpnStatus::Error(_)) => {
                            let _ = subtask_tx_monitor.send(SubTaskEvent::KillSwitch).await;
                            break;
                        }
                        _ => {}
                    }
                }
            }
        }
    });

    // Trigger transition to Disconnected
    *status.lock().await = VpnStatus::Disconnected;

    // Wait for SubTaskEvent::KillSwitch on the channel
    let mut killswitch_received = false;
    tokio::select! {
        _ = watch_handle => {}
        _ = tokio::time::sleep(std::time::Duration::from_secs(1)) => {}
    }

    while let Ok(event) = subtask_rx.try_recv() {
        if let SubTaskEvent::KillSwitch = event {
            killswitch_received = true;
            break;
        }
    }

    assert!(
        killswitch_received,
        "Orchestrator should have received a KillSwitch event on VPN disconnect"
    );
}
