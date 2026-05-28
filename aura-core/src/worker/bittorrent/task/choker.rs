use crate::worker::bittorrent::task::BtTask;
use crate::Result;

impl BtTask {
    pub async fn run_choker_loop(
        &self,
        worker_cmd_tx: tokio::sync::broadcast::Sender<crate::orchestrator::WorkerCommand>,
        token: tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        let mut ticks = 0;
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                _ = interval.tick() => {}
            }

            ticks += 1;
            let optimistic = ticks % 3 == 0;
            self.tick_choker(optimistic, &worker_cmd_tx).await;
        }
        Ok(())
    }

    pub async fn tick_choker(
        &self,
        optimistic: bool,
        worker_cmd_tx: &tokio::sync::broadcast::Sender<crate::orchestrator::WorkerCommand>,
    ) {
        let mut registry = self.state.registry.lock().await;
        registry.tick_rates(10.0);

        if optimistic {
            registry.reset_optimistic_unchokes();
        }

        let is_seeding = self
            .state
            .bitfield
            .lock()
            .await
            .as_ref()
            .map(|bf| bf.is_complete())
            .unwrap_or(false);

        let mut peers: Vec<_> = registry.get_all_connected().into_iter().collect();

        // Shuffle peers first so that stable sort keeps tied peers in random order
        use rand::seq::SliceRandom;
        peers.shuffle(&mut rand::rng());

        // Sort peers:
        // If we are leeching: unchoke peers we download fastest from.
        // If we are seeding: unchoke peers we upload fastest to.
        peers.sort_by(|a, b| {
            if is_seeding {
                b.upload_rate.partial_cmp(&a.upload_rate)
            } else {
                b.download_rate.partial_cmp(&a.download_rate)
            }
            .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut unchoked_count = 0;
        let mut candidates_for_optimistic = Vec::new();

        for p in peers.iter_mut() {
            // tit-for-tat unchokes the top 4 INTERESTED peers.
            if unchoked_count < 4 && p.peer_interested {
                if p.am_choking {
                    p.am_choking = false;
                    let _ = worker_cmd_tx.send(crate::orchestrator::WorkerCommand::Unchoke(
                        p.peer.ip.clone(),
                        p.peer.port,
                    ));
                }
                unchoked_count += 1;
            } else {
                if p.is_optimistic_unchoke {
                    unchoked_count += 1;
                    continue;
                }

                if p.am_choking {
                    if p.peer_interested {
                        candidates_for_optimistic.push(p);
                    }
                } else {
                    // This peer was unchoked but is no longer in the top 4.
                    p.am_choking = true;
                    let _ = worker_cmd_tx.send(crate::orchestrator::WorkerCommand::Choke(
                        p.peer.ip.clone(),
                        p.peer.port,
                    ));
                }
            }
        }

        // Handle Optimistic Unchoke
        if optimistic && !candidates_for_optimistic.is_empty() {
            // Since candidates are already shuffled, we can just pick the first one
            let p = &mut candidates_for_optimistic[0];
            p.am_choking = false;
            p.is_optimistic_unchoke = true;
            let _ = worker_cmd_tx.send(crate::orchestrator::WorkerCommand::Unchoke(
                p.peer.ip.clone(),
                p.peer.port,
            ));
        }
    }
}
