---
title      : "Feat: Implement PEX (Peer Exchange) — BEP 11"
labels     : [type:enhancement, priority:moderate, area:bt-worker]
description: |
  The config has `pex_enabled: bool` and the README mentions PEX as a feature, but there is zero PEX implementation anywhere in the codebase. PEX (BEP 11) allows peers to exchange lists of known peers, significantly improving swarm discovery speed.

  Discovered during the 2026-05-24 code-level deep-dive audit.

  Acceptance criteria:
  - Register `ut_pex` in the BEP 10 extended handshake.
  - Periodically send PEX messages containing recently connected/disconnected peers (60-second interval).
  - Process incoming PEX messages and add discovered peers to the peer registry.
  - Respect the `pex_enabled` config flag.
  - Add unit tests for PEX message encoding/decoding.
---
