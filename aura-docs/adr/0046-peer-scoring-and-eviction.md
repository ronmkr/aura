Status: Implemented

# ADR 0046: Peer Registry Health Scoring & Eviction

## Status
Implemented (2026-05-30, PR #165)

## Context
As a BitTorrent swarm grows, the `PeerRegistry` can accumulate a large number of inactive, slow, or error-prone peers. Unbounded growth of the peer list increases memory usage and connection overhead. We need a systematic way to evaluate peer health and evict underperforming peers when capacity limits are reached.

## Decision
We implement a dynamic scoring and eviction policy within the `PeerRegistry`:

1. **Capacity Limit**: The `PeerRegistry` maintains a hard limit of `MAX_PEERS` (e.g., 500).
2. **Health Scoring**: Each `PeerState` is assigned a dynamic score calculated as:
   `score = (download_rate_bytes_sec) - (error_count * 10.0) - (idle_seconds * 0.5)`
   This formula rewards high-throughput peers while aggressively penalizing peers that frequently fail or remain idle for extended periods.
3. **Eviction Policy (Bottom 10%)**: When `add_peers()` is called and the registry size exceeds `MAX_PEERS`, the peers are sorted by their current health score. The bottom 10% of peers (e.g., 50 peers) are immediately evicted to make room for new candidates. Active connections for these evicted peers are subsequently terminated.

## Consequences
- **Pros**: Ensures the engine prioritizes high-quality peers and prevents memory bloat in massive swarms.
- **Cons**: A peer that is temporarily slow might be unfairly evicted, requiring it to be rediscovered later if needed. The scoring formula may require tuning based on real-world swarm behaviors.

## Implementation
- **Peer Scoring & Eviction**: Implemented in `aura-core/src/bt_worker/peer_registry/` (2026-05-30, PR #165).
