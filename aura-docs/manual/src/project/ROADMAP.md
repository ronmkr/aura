# Aura: Project Tasks & Roadmap

> Last updated: 2026-05-26 (post-audit & gap-fix session)

## Milestone 1: The Atomic Download (Completed)
- [x] Basic Actor Skeleton (Orchestrator, StorageEngine).
- [x] Protocol Abstraction (`ProtocolWorker` trait).
- [x] HTTP Single-source retrieval.
- [x] Engine API for high-level control.
- [x] Basic CLI Persona (`Aura-cli`).

## Milestone 2: The "Smart" Buffer (Completed)
- [x] Real Disk I/O with `tokio::fs`.
- [x] Atomic Completion (`.part` file rename logic).
- [x] Sequential Write Aggregation (in-memory reordering).
- [x] Buffer Pool (memory reuse).
- [x] Bi-directional Actor Signaling (Storage -> Orchestrator completion).

## Milestone 3: The Swarm (BitTorrent) (Completed)
- [x] **Bitfield Implementation** (TDD: Unit tests first).
- [x] **Piece Selection Logic** (Rarest-First strategy).
- [x] **BitTorrent Protocol Worker** (Handshake, Message parsing, Codec).
- [x] **Torrent File Parsing** (Bencode, InfoHash).
- [x] **Tracker Client** (Robust UDP, HTTP Announce, Multi-IP support).
- [x] **Piece Hash Verification** (SHA-1 + SHA-256 Merkle for v2).
- [x] **Peer Registry** (Rate tracking and tit-for-tat support).
- [x] **Request Pipelining** (High-speed concurrent requests, pipeline_size=10).
- [x] **DHT Actor** (Token rotation and period node refresh).
- [x] **Magnet Link Support** (BEP 9 Metadata Exchange).
- [x] **Local Peer Discovery (LPD)** (BEP 14 Multicast).

## Milestone 4: Hyper-Scale (Aggregator) (Completed)
- [x] **Sourced Aggregator** (Multi-protocol task merging).
- [x] **Work Stealing & Racing** (EWMA-based speculative execution in `task/logic.rs`).
- [x] **Endgame Mode** (Parallel final block fetching, <1% or 3 pieces trigger).
- [x] **Adaptive Connection Scaling** (Spawns connections when throughput/connection < 256 KB/s).
- [x] **Hierarchical Throttling** (Semaphore-based token bucket, global + per-task).

## Milestone 5: Personas & UX (Completed)
- [x] **RPC Server** (Axum/JSON-RPC 2.0: addUri, tellActive, pause, unpause, remove, getConfig).
- [x] **Themeable TUI** (Ratatui + JSON-RPC client, 8 theme tokens from config).
- [x] **Public Rust API** (TaskHandle with filtered event stream).
- [x] **Browser Bridge** (`/extension/add` endpoint with MIME-type detection).
- [x] **Headless Daemon Mode** (`Aura-daemon`).
- [x] **Web UI Dashboard** (Embedded SPA via `rust-embed`, Catppuccin dark theme).

## Milestone 6: Persistence & Advanced Protocols (Completed)
- [x] **Pause/Resume Support** (Cancellation tokens, state persistence).
- [x] **Task Persistence** (`.aura` JSON control files + sled DB for bitfields).
- [x] **NAT Traversal** (UPnP/NAT-PMP with 30-min refresh cycle).
- [x] **BitTorrent v2** (Merkle-tree SHA-256, hybrid v1+v2 torrents).
- [x] **URL Globbing & Batch Downloads** (Ranges, sets, zero-pad, nested expansion).
- [x] **FTP/FTPS Protocol Support** (TLS support + exponential retry).
- [x] **VPN Safety & Traffic Kill-switch** (Tunnel monitoring and mandatory interface binding).
- [x] **Metalink Support (V3/V4)** (Priority parsing and multi-source coordination).
- [x] **Dynamic TOML Configuration** (Hot-reload via `notify` in engine.rs + `arc-swap`).
- [x] **Modular Architecture Refactor** (Strict 400-line per file limit enforced).
- [x] **Unified Binary Architecture** (Single `aura` executable with subcommands).
- [x] **Power Management** (Thread-isolated `nosleep`, cross-platform).

## Milestone 7: Industrial Hardening (In Progress)

### Completed
- [x] **Non-Swarm Integrity** (MD5/SHA-1/SHA-256/SHA-512 checksum at storage layer).
- [x] **Unified Credential Provider** (Netrc + Netscape cookie jar).
- [x] **No-COW Allocator** (Btrfs `FS_NOCOW_FL` ioctl + ZFS detection).
- [x] **Policy-based Error Management** (Retry backoff, mirror failover, URI blacklisting).
- [x] **MIME Validation & Landing Page Resolution** (HTML scraping for asset links).
- [x] **fsync before atomic rename** (Prevents data loss on system crash).
- [x] **DHT token security** (Token rotation per BEP 5).
- [x] **BT choking algorithm** (Standard tit-for-tat + optimistic unchoke).
- [x] **BDD Test Implementation** (Realistic integration coverage for storage, swarm, and daemon).
- [x] **BT block size fix** (Standardized to 16KB).
- [x] **DNS over HTTPS (DoH/DoT)** (Privacy-enhanced asynchronous resolution).
- [x] **Integrity Scrubbing & Self-healing Storage** (Issue #4).
- [x] **Generational Write Buffer & Advanced Caching** (Issue #14).
- [x] **PEX implementation** (BEP 11 Peer Exchange) (Issue #121).
- [x] **Peer registry scoring** (Latency/Reputation/Eviction) (Issue #123).
- [x] **Advanced Disk I/O Scheduling** (Deadline-based writes) (Issue #13).
- [x] **Advanced Networking (kTLS, Captive Portals, Roaming)** (Issue #8).
- [x] **Multi-tenancy & Structured Audit Tracing** (Issue #15).
- [x] **Architectural Decoupling (God Nodes)** (Decision-0072).
- [x] **Cloud Storage Support (S3, GDrive, OneDrive)** (Issue #10, Decision 0013) (#280).

### Remaining - Priority

### Future Backlog
- [ ] **Recursive Mirroring** (Wget-style site crawling) (Issue #65).
- [ ] **Network Filesystem (NFS/SMB) Optimizations** (Issue #12).
- [ ] **Task Chaining & Metadata-based Path Mapping** (Issue #11).
- [ ] **HTTP/3 QUIC** (Issue #23).
- [ ] **QR Code Sharing** (Issue #74).
- [ ] **i18n Architecture** (Issue #71).

## Milestone 8: Daemon Maturity & Production Swarm Capabilities (In Progress)
- [x] **feat: Implement MSE/PE (Message Stream Encryption) for BitTorrent traffic obfuscation** (Issue #283, Decision-0066).
- [ ] **feat: Implement μTP/LEDBAT transport (BEP 29) for ISP-friendly BitTorrent** (Issue #286, Decision-0067).
- [x] **feat: Fast resume — verify and reuse existing file data on task re-add** (Issue #284, Decision-0068).
- [x] **feat: Watch folder — auto-ingest torrents/metalinks** (Issue #288, Decision-0069).
- [x] **feat: RSS/Atom feed subscriptions for automated download ingestion** (Issue #290, Decision-0070).
- [x] **feat: System service integration (systemd, launchd, Windows Service)** (Issue #291, Decision-0071).
- [x] **feat: Implement BitTorrent tracker scrape for swarm statistics** (Issue #289).


## Infrastructure & DevSecOps
- [x] `GEMINI.md` (Engineering Mandates).
- [x] `design.md` (System Design & Visuals).
- [x] `CONTEXT.md` (Ubiquitous Language).
- [x] `.github/workflows/ci.yml` (fmt + clippy + test).
- [x] `.github/workflows/codeql.yml` (Weekly security scan).
- [x] Automated Benchmarking Suite (buffer_bench + storage_bench).
- [x] Comprehensive Integration Tests (Cucumber BDD suite).
- [x] Automated Binary Releases (GitHub Actions matrix builds).
- [x] CI Docs Deployment (Issue #134).
