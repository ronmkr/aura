---
title      : "Bug: VPN kill-switch (force_tunnel) is dead code — not enforced"
labels     : [type:bug, priority:high, area:vpn]
description: |
  The `VpnConfig` struct has a `force_tunnel: bool` field, and the orchestrator has `verify_vpn_connectivity()` logic, but the actual kill-switch enforcement is incomplete:
  - `force_tunnel` is never read or checked in `vpn/logic.rs`.
  - No firewall rules (iptables/pf) are applied to prevent traffic leaks.
  - Interface binding from `VpnProvider::interface()` is not automatically wired to socket creation.

  Users who rely on `force_tunnel = true` to prevent non-VPN traffic have a false sense of security.

  Discovered during the 2026-05-24 code-level deep-dive audit.

  Acceptance criteria:
  - When `force_tunnel = true`, bind all outgoing sockets to the VPN interface returned by `VpnProvider::interface()`.
  - If the VPN interface goes down, immediately pause all active tasks (the orchestrator's `KillSwitch` command already exists).
  - Add integration test verifying that downloads fail when VPN is down and `force_tunnel = true`.
---
