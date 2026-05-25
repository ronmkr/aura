---
title      : "Feat: Add peer health scoring, reputation, and eviction to peer registry"
labels     : [type:enhancement, priority:moderate, area:peer-registry]
description: |
  The peer registry (`peer_registry/logic.rs`, 115 lines) currently only tracks connection state (4-state enum). It lacks:
  - Download/upload speed tracking per peer
  - Latency measurement
  - Error/corrupt piece counting
  - Reputation scoring based on data contributed
  - Peer banning on corrupt data
  - Max peer eviction of worst performers
  - Intelligent peer selection (currently picks first Disconnected peer)

  Discovered during the 2026-05-24 code-level deep-dive audit.

  Acceptance criteria:
  - Track bytes downloaded/uploaded per peer with EWMA speed calculation.
  - Track error count and corrupt piece count per peer.
  - Compute a reputation score based on (speed * reliability).
  - Ban peers that send >2 corrupt pieces.
  - When max_peers is reached, evict the lowest-reputation peer.
  - `get_peer_to_connect()` should prefer higher-reputation peers.
---
