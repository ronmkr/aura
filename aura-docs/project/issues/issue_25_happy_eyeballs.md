---
title      : "Feat: Complete Dual-Stack Asynchronous DNS Racing (Happy Eyeballs)"
labels     : [type:enhancement, priority:moderate, area:network]
status     : RESOLVED
resolved   : 2026-05-17
description: |
  While `connect_tcp_bound` implements `race_connect` over already-resolved `SocketAddr` vectors, the DNS resolution phase itself is still blocking/synchronous and does not perform dual-stack DNS racing (RFC 8305) to resolve hostnames to IPv4 and IPv6 addresses simultaneously.

  We should accept raw `(host, port)` in `connect_tcp_bound` and asynchronously race resolving and connecting to dual-stack hosts.

  Acceptance criteria:
  - Accept dual-stack address lists or a `(String, u16)` target in `connect_tcp_bound`.
  - Asynchronously perform IPv4 and IPv6 lookup queries in parallel.
  - Implement staggered connection attempts (250ms gap) between IPv6 and IPv4 addresses to favor IPv6 while maintaining quick fallback.

  Resolution: Implemented in `net_util/logic.rs::connect_tcp_bound_host()` — DNS resolution → interleaves IPv6/IPv4 addresses → races with 250ms stagger via `FuturesUnordered`. Not full RFC 8305 (missing resolution delay, sorting) but functionally correct.
---
