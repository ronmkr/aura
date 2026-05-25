---
title      : "Bug: Daemon ignores rpc_port config — hardcoded to 6800"
labels     : [type:bug, priority:low, area:daemon]
description: |
  `aura-daemon/src/main.rs` hardcodes the server bind address to `0.0.0.0:6800`, ignoring the `rpc_port` value from `Aura.toml`'s `[network]` section.

  Discovered during the 2026-05-24 code-level deep-dive audit.

  Acceptance criteria:
  - Read `config.network.rpc_port` and use it for the Axum server bind address.
  - Default to 6800 if not specified.
  - Log the bound address on startup.
---
