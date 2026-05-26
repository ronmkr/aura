# ADR 0044: BitTorrent Choking Algorithm (Tit-for-Tat)

## Status
Proposed

## Context
Aura's BitTorrent worker was acting as a pure leecher, requesting pieces from all peers without ever unchoking them in return. This is inefficient for the swarm and leads to Aura being snubbed or banned by other clients that enforce tit-for-tat fairness. To participate fully in the BitTorrent ecosystem, Aura needs a standard choking algorithm.

## Decision
We will implement the standard BitTorrent choking algorithm based on tit-for-tat and optimistic unchoking:

1.  **Tit-for-Tat (10s Cycle)**:
    -   Track the download rate from each peer in the `PeerRegistry`.
    -   Every 10 seconds, sort all connected peers by their average download rate over the last interval.
    -   Unchoke the top 4 peers to reward them for providing data.
    -   Choke all other peers (except the optimistic unchoke).

2.  **Optimistic Unchoke (30s Cycle)**:
    -   Every 3rd cycle (30 seconds), pick one additional peer at random from the choked set and unchoke it.
    -   This allows Aura to discover new peers that might have better bandwidth than the current top 4.

3.  **Communication**:
    -   The `BtTask` will run a central `run_choker_loop`.
    -   It will dispatch `WorkerCommand::Choke` and `WorkerCommand::Unchoke` to individual `BtWorker` actors.
    -   `BtWorker` actors will translate these commands into wire-level `PeerMessage::Choke/Unchoke`.

## Consequences
-   **Improved Swarm Performance**: Aura will be unchoked by more peers as it starts providing data back.
-   **Resource Usage**: Slight increase in CPU and memory to track per-peer bandwidth and run the 10s timer loop.
-   **Fairness**: Aura now adheres to the BEP 3 specification for peer interest and choking.
