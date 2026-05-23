---
name: "Feature request: Happy Eyeballs DNS Resolution Racing"
about: Asynchronously race dual-stack IPv4/IPv6 address resolutions staggering by 250ms.
title: "Feat: Complete Dual-Stack Asynchronous DNS Racing (Happy Eyeballs)"
labels: ["type:enhancement", "priority:moderate", "area:network"]
assignees: ""
---

### Problem Description
While `connect_tcp_bound` implements `race_connect` over already-resolved `SocketAddr` vectors, the DNS resolution phase itself is still blocking/synchronous and does not perform dual-stack DNS racing (RFC 8305) to resolve hostnames to IPv4 and IPv6 addresses simultaneously.

We should accept raw `(host, port)` in `connect_tcp_bound` and asynchronously race resolving and connecting to dual-stack hosts.

### Proposed Solution
- Accept dual-stack address lists or a `(String, u16)` target in `connect_tcp_bound`.
- Asynchronously perform IPv4 and IPv6 lookup queries in parallel.
- Implement staggered connection attempts (250ms gap) between IPv6 and IPv4 addresses to favor IPv6 while maintaining quick fallback.

### Acceptance Criteria
- [ ] Extend `connect_tcp_bound` to accept a host and port string.
- [ ] Run parallel asynchronous DNS queries for A and AAAA records.
- [ ] Stagger connection attempts by 250ms to favor IPv6.
- [ ] Validate dual-stack connectivity fallback in integration tests.
