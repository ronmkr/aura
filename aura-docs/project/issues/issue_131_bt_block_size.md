---
title      : "Bug: BT block size is 32KB — non-standard (spec is 16KB)"
labels     : [type:bug, priority:low, area:bt-worker]
description: |
  `bt_worker/protocol.rs` line 8 defines `BLOCK_SIZE = 32 * 1024` (32KB). The BitTorrent specification and most clients use 16KB (16384 bytes) as the standard request block size. Using 32KB may cause compatibility issues with strict peers that reject non-standard request sizes.

  Discovered during the 2026-05-24 code-level deep-dive audit.

  Acceptance criteria:
  - Change `BLOCK_SIZE` to `16 * 1024` (16KB) to match the BT specification.
  - Verify existing tests still pass.
  - Add a note in CONTEXT.md about the block size choice.
---
