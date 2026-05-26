# Aura: Development Tasks

All active development tasks, technical debt, and feature requests are managed exclusively via [GitHub Issues](https://github.com/ronmkr/aura/issues).

## 🟢 Open Tasks

- [ ] **infra: CI Docs Deployment** (Issue #134)
- [ ] **Feat: Respect BEP 12 tracker tier ordering** (Issue #128) `[enhancement]`
- [ ] **Test: Implement stub BDD step definitions for daemon, networking, storage, swarm features** (Issue #124) `[status:unverified]`
- [ ] **Feat: Add peer health scoring, reputation, and eviction to peer registry** (Issue #123) `[enhancement, module:core, priority:moderate]`
- [ ] **Feat: Implement PEX (Peer Exchange) — BEP 11** (Issue #121) `[enhancement, module:core, priority:moderate]`
- [ ] **Feat: Implement BT choking algorithm (tit-for-tat + optimistic unchoke)** (Issue #120) `[enhancement, module:core, priority:critical]`
- [ ] **feat: QR code sharing for magnet links in CLI/TUI** (Issue #74)
- [ ] **feat: implement Privacy-Enhanced Resolver (DoH/DoT)** (Issue #73)
- [ ] **feat: task prioritization and dependency management** (Issue #72) `[enhancement, module:core, priority:moderate]`
- [ ] **feat: i18n support for CLI and TUI** (Issue #71)
- [ ] **feat: implement recursive mirroring (wget parity)** (Issue #65) `[enhancement, module:core, priority:moderate]`
- [ ] **feat: Prioritized Streaming Mode for Media Playback** (Issue #28) `[enhancement, module:core, priority:moderate]`
- [ ] **feat: Modern Networking: HTTP/3 (QUIC) & Alt-Svc Support** (Issue #23) `[enhancement, module:core, priority:moderate]`
- [ ] **feat: NNTP (Usenet) Protocol Support** (Issue #22)
- [ ] **feat: Multi-tenancy & Structured Audit Tracing** (Issue #15) `[enhancement, priority:moderate, module:daemon]`
- [ ] **perf: Generational Write Buffer & Advanced Caching** (Issue #14) `[enhancement, module:storage, priority:moderate]`
- [ ] **perf: Advanced Disk I/O Scheduling** (Issue #13) `[enhancement, module:storage, priority:moderate]`
- [ ] **feat: Network Filesystem (NFS/SMB) Optimizations** (Issue #12)
- [ ] **feat: Task Chaining & Metadata-based Path Mapping** (Issue #11)
- [ ] **feat: Cloud Storage Support (S3, Google Drive)** (Issue #10) `[enhancement, module:core, priority:moderate]`
- [ ] **feat: Advanced Networking (kTLS, Captive Portals, Roaming)** (Issue #8)
- [ ] **feat: Integrity Scrubbing & Self-healing Storage** (Issue #4) `[enhancement, module:storage, priority:moderate]`

## ✅ Completed Tasks

- [x] **Bug: DNS resolver config is a facade — DoH/DoT not wired** (Issue #135)
- [x] **Bug: BT block size is 32KB — non-standard (spec is 16KB)** (Issue #127) `[bug]`
- [x] **Bug: Metalink parser has debug eprintln and hardcoded priority** (Issue #126) `[bug]`
- [x] **Bug: Daemon ignores rpc_port config — hardcoded to 6800** (Issue #125) `[bug]`
- [x] **Feat: Add FTPS (TLS) support and retry logic to FTP worker** (Issue #122) `[enhancement]`
- [x] **Bug: VPN kill-switch (force_tunnel) is dead code — not enforced** (Issue #119) `[bug]`
- [x] **Bug: DHT token is hardcoded [1,2,3,4] — security vulnerability** (Issue #118) `[bug]`
- [x] **Bug: Add fsync/fdatasync before atomic .part rename to prevent data loss** (Issue #117) `[bug, module:storage]`
- [x] **Feature: NAT Traversal Mapping Refresh** (Issue #114) `[enhancement]`
- [x] **feat: WebSocket Telemetry for RPC (ADR 0016 Edge Case)** (Issue #104)
- [x] **chore: Modular Architecture Refactor (exceeding 400 lines)** (Issue #96)
- [x] **feat: Browser Bridge (Extension Support)** (Issue #95)
- [x] **feat: implement advanced filesystem hardening (Pre-allocation/No-COW)** (Issue #92) `[module:storage, status:unverified]`
- [x] **Implement Integration Tests Suite** (Issue #89) `[status:unverified]`
- [x] **refactor: technical debt cleanup and dead code removal** (Issue #78)
- [x] **feat: dynamic DHT bootstrap node refreshing** (Issue #77)
- [x] **feat: enforce BitTorrent upload ratio and seed time limits** (Issue #76)
- [x] **feat: checksum verification for HTTP and FTP downloads** (Issue #75)
- [x] **feat: implement COW-aware storage allocator (Btrfs/ZFS)** (Issue #70)
- [x] **feat: prometheus metrics and telemetry exporter** (Issue #69) `[enhancement]`
- [x] **feat: support BitTorrent v2 (SHA-256 Merkle Trees)** (Issue #68)
- [x] **feat: built-in Web UI dashboard for Aura Daemon** (Issue #67)
- [x] **feat: implement SOCKS5 proxy support for BitTorrent protocol** (Issue #66)
- [x] **feat: enable full palette customization & token-based theming** (Issue #57)
- [x] **refactor: Use robust URL encoding for tracker announcements** (Issue #52)
- [x] **perf: Wrap synchronous network interface calls in spawn_blocking** (Issue #51)
- [x] **perf: Replace unbounded subtask channel with bounded backpressure** (Issue #50)
- [x] **reliability: Replace unchecked unwrap() calls with robust error handling** (Issue #49)
- [x] **infra: Automated Performance Benchmarking Suite** (Issue #47)
- [x] **feat: Public Rust API & Embeddability (TaskHandles)** (Issue #46)
- [x] **feat: Magnet Link Support (BEP 9 Metadata Exchange)** (Issue #45)
- [x] **feat: Task Persistence & Session Recovery (.aura files)** (Issue #44)
- [x] **feat: URL Globbing & Batch Expansion Support** (Issue #43)
- [x] **feat: Native VPN Integration (OpenVPN/WireGuard)** (Issue #42)
- [x] **chore: Modular Architecture Refactor (No file > 400 lines)** (Issue #41)
- [x] **feat: Metalink Support (V3/V4) & Automated Multi-source Tasking** (Issue #33)
- [x] **feat: Unified Proxy Connector (SOCKS5/HTTP)** (Issue #32)
- [x] **feat: Adaptive Connection Scaling & Sourced Aggregation** (Issue #31) `[status:stub, module:core, module:storage]`
- [x] **feat: Hierarchical Token Bucket Throttling** (Issue #30) `[status:stub, module:core, module:storage]`
- [x] **feat: Racing Work Stealer for Slow Stream Mitigation** (Issue #29) `[status:stub, module:core, module:storage]`
- [x] **feat: Themeable TUI & UI Customization** (Issue #27)
- [x] **perf: Write-Back Caching & Memory-aligned I/O** (Issue #26)
- [x] **feat: Happy Eyeballs (RFC 8305) Support** (Issue #25)
- [x] **feat: DNS over HTTPS (DoH) & Async DNS Resolver** (Issue #24)
- [x] **feat: Credential Provider & Security Abstraction** (Issue #21)
- [x] **feat: HSTS Cache & Automated HTTPS Upgrade** (Issue #20)
- [x] **feat: MIME Validation & Landing Page Resolution** (Issue #19) `[enhancement]`
- [x] **feat: Policy-based Error Management & Self-healing** (Issue #18) `[enhancement]`
- [x] **feat: Integrated Hook System (Event Callbacks)** (Issue #16)
- [x] **feat: BitTorrent v2 Support (Merkle Trees)** (Issue #9)
- [x] **feat: Recursive Site Mirroring (Wget-style)** (Issue #7)
- [x] **feat: BitTorrent Endgame Mode** (Issue #6)
- [x] **feat: Power Management & Sleep Prevention** (Issue #5)
- [x] **feat: Dynamic TOML Configuration & Hot-reloading** (Issue #3)
- [x] **perf: No-COW Allocator & Disk Optimization** (Issue #2)
- [x] **feat: Local Peer Discovery (LPD) Support** (Issue #1)
