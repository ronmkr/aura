---
title      : "Feat: Respect BEP 12 tracker tier ordering"
labels     : [type:enhancement, priority:low, area:tracker]
description: |
  The tracker client (`tracker/logic.rs`) processes all trackers from `announce_list` in parallel via `join_all`. BEP 12 specifies that trackers should be contacted by tier — only moving to the next tier if the current tier fails.

  Discovered during the 2026-05-24 code-level deep-dive audit.

  Acceptance criteria:
  - Process announce_list tiers sequentially (tier 0 first, then tier 1 if tier 0 fails, etc.).
  - Within a tier, contact trackers in parallel.
  - Promote successful trackers to the front of their tier (BEP 12 randomization).
---
