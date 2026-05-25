---
title      : "Feat: Implement BT choking algorithm (tit-for-tat + optimistic unchoke)"
labels     : [type:enhancement, priority:high, area:bt-worker]
description: |
  The BT worker currently tracks `peer_choking` state from remote peers but never sends outgoing Choke/Unchoke/NotInterested messages. This means:
  - The client cannot manage its upload bandwidth to leechers.
  - There is no tit-for-tat incentive mechanism.
  - No optimistic unchoke cycle to discover fast peers.

  The peer_registry also has `am_choking`, `am_interested`, `peer_choking`, `peer_interested` fields that are initialized but never updated.

  Discovered during the 2026-05-24 code-level deep-dive audit.

  Acceptance criteria:
  - Implement a 10-second unchoke cycle: unchoke the top 4 peers by download rate (tit-for-tat).
  - Implement a 30-second optimistic unchoke: randomly unchoke one additional choked peer.
  - Send Choke/Unchoke messages to peers based on the algorithm decisions.
  - Track and update `am_choking`/`am_interested` in the peer registry.
  - Send NotInterested when no pieces are needed from a peer.
---
