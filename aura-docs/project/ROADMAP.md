# Aura: Project Tasks & Roadmap

> Last updated: 2026-05-24 (post code-level deep-dive audit)

## ✅ Milestone 1: The Atomic Download (Completed)
- [x] Basic Actor Skeleton (Orchestrator, StorageEngine).
- [x] Protocol Abstraction (`ProtocolWorker` trait).
- [x] HTTP Single-source retrieval.
- [x] Engine API for high-level control.
- [x] Basic CLI Persona (`Aura-cli`).

## ✅ Milestone 2: The "Smart" Buffer (Completed)
- [x] Real Disk I/O with `tokio::fs`.
- [x] Atomic Completion (`.part` file rename logic).
- [x] Sequential Write Aggregation (in-memory reordering).
- [x] Buffer Pool (memory reuse).
- [x] Bi-directional Actor Signaling (Storage -> Orchestrator completion).

## ✅ Milestone 3: The Swarm (BitTorrent) — Completed (with caveats)
- [x] **Bitfield Implementation** (TDD: Unit tests first).
- [x] **Piece Selection Logic** (Rarest-First strategy).
- [x] **BitTorrent Protocol Worker** (Handshake, Message parsing, Codec).
- [x] **Torrent File Parsing** (Bencode, InfoHash).
- [x] **Tracker Client** (Robust UDP, HTTP Announce, Multi-IP support).
- [x] **Piece Hash Verification** (SHA-1 + SHA-256 Merkle for v2).
- [x] **Peer Registry** (Basic state tracking — ⚠️ no scoring/reputation, Issue #126).
- [x] **Request Pipelining** (High-speed concurrent requests, pipeline_size=10).
- [ ] **DHT/PEX Actors** — DHT ✅ real (but token hardcoded, Issue #121). **PEX ⬜ not implemented** (Issue #124).
- [ ] **Seeding Mode** — Passive upload ✅ (responds to Request). ⬜ No proactive seeding mode switch.
- [x] **Magnet Link Support** (BEP 9 Metadata Exchange).
- [x] **Local Peer Discovery (LPD)** (BEP 14 Multicast).

> [!WARNING]
> **Deep-dive corrections**: PEX has zero implementation despite config flag. Choking algorithm (tit-for-tat, optimistic unchoke) is completely missing (Issue #123). Peer registry is minimal (Issue #126). BT block size is non-standard 32KB (Issue #131).

## ✅ Milestone 4: Hyper-Scale (Aggregator) (Completed)
- [x] **Sourced Aggregator** (Multi-protocol task merging).
- [x] **Work Stealing & Racing** (EWMA-based speculative execution in `task/logic.rs`).
- [x] **Endgame Mode** (Parallel final block fetching, <1% or ≤3 pieces trigger).
- [x] **Adaptive Connection Scaling** (Spawns connections when throughput/connection < 256 KB/s).
- [x] **Hierarchical Throttling** (Semaphore-based token bucket, global + per-task).

## ✅ Milestone 5: Personas & UX (Completed)
- [x] **RPC Server** (Axum/JSON-RPC 2.0: addUri, tellActive, pause, unpause, remove, getConfig).
- [x] **Themeable TUI** (Ratatui + JSON-RPC client, 8 theme tokens from config).
- [x] **Public Rust API** (TaskHandle with filtered event stream).
- [x] **Browser Bridge** (`/extension/add` endpoint with MIME-type detection).
- [x] **Headless Daemon Mode** (`Aura-daemon`, ⚠️ port hardcoded to 6800, Issue #129).
- [x] **Web UI Dashboard** (Embedded SPA via `rust-embed`, Catppuccin dark theme).
- [ ] **QR Code Sharing** (CLI/TUI magnet link sharing) (Issue #74).

## ✅ Milestone 6: Persistence & Advanced Protocols (Completed — with caveats)
- [x] **Pause/Resume Support** (Cancellation tokens, state persistence).
- [x] **Task Persistence** (`.aura` JSON control files + sled DB for bitfields).
- [x] **NAT Traversal** (UPnP/NAT-PMP with 30-min refresh cycle).
- [x] **BitTorrent v2** (Merkle-tree SHA-256, hybrid v1+v2 torrents).
- [x] **URL Globbing & Batch Downloads** (Ranges, sets, zero-pad, nested expansion).
- [x] **FTP Protocol Support** (⚠️ No FTPS/TLS, no retry logic — Issue #125).
- [ ] **VPN Safety & Traffic Kill-switch** — Detection ✅ (WG/OVPN/Interface). ⬜ **Kill-switch is dead code** (Issue #122).
- [x] **Metalink Support (V3/V4)** (⚠️ Priority hardcoded 0, debug eprintln — Issue #130).
- [x] **Dynamic TOML Configuration** (Hot-reload via `notify` in engine.rs + `arc-swap`).
- [x] **Modular Architecture Refactor** (No file >400 lines, except http.rs at 625).
- [x] **Power Management** (Thread-isolated `nosleep`, cross-platform).

> [!WARNING]
> **Deep-dive corrections**: VPN kill-switch `force_tunnel` is dead code. FTP has no TLS and no retry. Metalink priority not parsed. Config hot-reload works but is in engine.rs, not config/logic.rs.

## 🚀 Milestone 7: Industrial Hardening (In Progress)

### Completed
- [x] **Non-Swarm Integrity** (MD5/SHA-1/SHA-256/SHA-512 checksum at storage layer) (Issue #75).
- [x] **Unified Credential Provider** (Netrc + Netscape cookie jar) (Issue #21).
- [x] **No-COW Allocator** (Btrfs `FS_NOCOW_FL` ioctl + ZFS detection) (Issue #92).
- [x] **Policy-based Error Management** (Retry backoff, mirror failover, URI blacklisting) (Issue #18).
- [x] **MIME Validation & Landing Page Resolution** (HTML scraping for asset links) (Issue #19).

### New — Critical (from Deep-Dive Audit)
- [ ] **fsync before atomic rename** (Issue #120) [CRITICAL].
- [ ] **DHT token security** (Issue #121) [CRITICAL].
- [ ] **VPN kill-switch enforcement** (Issue #122) [HIGH].
- [ ] **DNS resolver facade fix** (Issue #128) [MODERATE].

### New — Protocol Correctness (from Deep-Dive Audit)
- [ ] **BT choking algorithm** (Issue #123) [HIGH].
- [ ] **PEX implementation** (Issue #124) [MODERATE].
- [ ] **Peer registry scoring** (Issue #126) [MODERATE].
- [ ] **FTP: FTPS + retry** (Issue #125) [MODERATE].
- [ ] **BDD stub tests** (Issue #127) [MODERATE].
- [ ] **BT block size fix (32KB → 16KB)** (Issue #131) [LOW].
- [ ] **Tracker tier ordering** (Issue #132) [LOW].

### Existing — Not Started
- [ ] **Advanced Disk I/O Scheduling** (Deadline-based sorted writes) (Issue #13).
- [ ] **Multi-tenancy & Quotas** (Resource isolation) (Issue #15).
- [ ] **Recursive Mirroring** (Wget-style site crawling) (Issue #65).
- [ ] **Generational Write Buffer & Advanced Caching** (Issue #14).
- [ ] **Task Chaining & Metadata-based Path Mapping** (Issue #11).
- [ ] **Network Filesystem (NFS/SMB) Optimizations** (Issue #12).
- [ ] **Integrity Scrubbing & Self-healing Storage** (Issue #4).
- [ ] **Advanced Networking (kTLS, Captive Portals)** (Issue #8).

### Minor
- [ ] **Daemon port config** (Issue #129) [LOW].
- [ ] **Metalink cleanup** (Issue #130) [LOW].
- [ ] **QR Code Sharing** (Issue #74) [MINOR].
- [ ] **i18n Architecture** (Issue #71) [MINOR].
- [ ] **Task Priority Scheduling** (Issue #72) [MINOR].
- [ ] **Prioritized Streaming Mode** (Issue #28) [MINOR].
- [ ] **HTTP/3 QUIC** (Issue #23).
- [ ] **HSTS Cache** (Issue #20).
- [ ] **New Protocols** (NNTP #22, Cloud #10).

## 🛡️ Infrastructure & DevSecOps
- [x] `GEMINI.md` (Engineering Mandates).
- [x] `design.md` (System Design & Visuals).
- [x] `CONTEXT.md` (Ubiquitous Language).
- [x] `CONTRIBUTING.md`.
- [x] `.github/workflows/ci.yml` (fmt + clippy + test).
- [x] `.github/workflows/codeql.yml` (Weekly security scan).
- [x] Automated Benchmarking Suite (buffer_bench + storage_bench).
- [ ] Comprehensive Integration Tests — ⚠️ 4/11 BDD features have stub steps (Issue #127).
- [ ] CI Docs Deployment.
