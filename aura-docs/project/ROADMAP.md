# Aura: Project Tasks & Roadmap

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

## 🚀 Milestone 3: The Swarm (BitTorrent)
- [x] **Bitfield Implementation** (TDD: Unit tests first).
- [x] **Piece Selection Logic** (Rarest-First strategy).
- [x] **BitTorrent Protocol Worker** (Handshake, Message parsing, Codec).
- [x] **Torrent File Parsing** (Bencode, InfoHash).
- [x] **Tracker Client** (Robust UDP, HTTP Announce, Multi-IP support).
- [x] **Piece Hash Verification** (SHA-1 integrity checks).
- [x] **Peer Registry** (Managing peer state and lifecycle).
- [x] **Request Pipelining** (High-speed concurrent requests).
- [x] **DHT/PEX Actors** (Advanced peer discovery).
- [x] **Seeding Mode**.
- [x] **Magnet Link Support** (BEP 9 Metadata Exchange).
- [x] **Local Peer Discovery (LPD)** (Multicast discovery).

## ✅ Milestone 4: Hyper-Scale (Aggregator) (Completed)
- [x] **Sourced Aggregator** (Multi-protocol task merging).
- [x] **Work Stealing & Racing** (Speculative execution).
- [x] **Endgame Mode** (Parallel final block fetching).
- [x] **Adaptive Connection Scaling** (Bypassing per-connection caps).
- [x] **Hierarchical Throttling** (Global + Per-task speed limits).

## ✅ Milestone 5: Personas & UX (In Progress)
- [x] **RPC Server** (Axum/JSON-RPC 2.0).
- [x] **Themeable TUI** (Ratatui + JSON-RPC client).
- [x] **Public Rust API** (TaskHandles & Streams).
- [ ] **Browser Bridge** (Extension support).
- [x] **Headless Daemon Mode** (`Aura-daemon`).

## 🚀 Milestone 6: Persistence & Advanced Protocols
- [x] **Pause/Resume Support** (Stopping and restarting downloads).
- [x] **Task Persistence** (Control files for session recovery).
- [x] **NAT Traversal** (UPnP/NAT-PMP for seeding).
- [x] **BitTorrent v2** (Merkle-tree based integrity).
- [x] **URL Globbing & Batch Downloads**.
- [x] **FTP Protocol Support**.
- [x] **VPN Safety & Traffic Kill-switch** (ADR 0038).
- [x] **Metalink Support (V3/V4)** (Issue #33) - **[PRIORITY: HIGH]**.
- [x] **Dynamic TOML Configuration** (Aura.toml + Hot-reloading).
- [ ] **Modular Architecture Refactor** (No file > 400 lines) - **[PENDING: torrent.rs, dht/mod.rs]**.
- [ ] **VPN Native Integration** (OpenVPN/WireGuard support) (ADR 0038).
- [x] **Power Management** (OS-level sleep assertions).

## 🚀 Milestone 7: Industrial Hardening (Proposed)
- [ ] **Advanced Disk I/O Scheduling** (Deadline-based sorted writes) (ADR 0022).
- [ ] **Unified Credential Provider** (Netrc/Cookie management) (ADR 0014).
- [ ] **Multi-tenancy & Quotas** (Resource isolation) (ADR 0032).
- [ ] **No-COW Allocator** (Btrfs/ZFS fragmentation prevention).
- [ ] **Recursive Mirroring** (Wget-style site crawling).

## 🛡️ Infrastructure & DevSecOps
- [x] `GEMINI.md` (Engineering Mandates).
- [x] `design.md` (System Design & Visuals).
- [x] `CONTEXT.md` (Ubiquitous Language).
- [x] `CONTRIBUTING.md`.
- [x] `.github/workflows/ci.yml`.
- [x] Automated Benchmarking Suite.
- [x] Comprehensive Integration Tests.
