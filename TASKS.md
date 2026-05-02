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
- [ ] **Local Peer Discovery (LPD)** (Multicast discovery).

## ✅ Milestone 4: Hyper-Scale (Aggregator) (Completed)
- [x] **Sourced Aggregator** (Multi-protocol task merging).
- [x] **Work Stealing & Racing** (Speculative execution).
- [ ] **Endgame Mode** (Parallel final block fetching).
- [x] **Adaptive Connection Scaling** (Bypassing per-connection caps).
- [x] **Hierarchical Throttling** (Global + Per-task speed limits).

## ✅ Milestone 5: Personas & UX (In Progress)
- [x] **RPC Server** (Axum/JSON-RPC 2.0).
- [x] **Themeable TUI** (Ratatui + JSON-RPC client).
- [ ] **Browser Bridge** (Extension support).
- [x] **Headless Daemon Mode** (`Aura-daemon`).

## 🚀 Milestone 6: Persistence & Advanced Protocols
- [x] **Pause/Resume Support** (Stopping and restarting downloads).
- [x] **Task Persistence** (Control files for session recovery).
- [x] **NAT Traversal** (UPnP/NAT-PMP for seeding).
- [ ] **BitTorrent v2** (Merkle-tree based integrity).
- [x] **URL Globbing & Batch Downloads**.
- [x] **FTP Protocol Support**.
- [x] **VPN Safety & Traffic Kill-switch** (ADR 0035).
- [x] **Dynamic TOML Configuration** (Aura.toml + Hot-reloading).
- [x] **Modular Architecture Refactor** (No file > 400 lines).
- [ ] **VPN Native Integration** (OpenVPN/WireGuard support) (ADR 0038).
- [ ] **Power Management** (OS-level sleep assertions).
- [ ] **No-COW Allocator** (Btrfs/ZFS fragmentation prevention).
- [ ] **Recursive Mirroring** (Wget-style site crawling).

## 🛡️ Infrastructure & DevSecOps
- [x] `GEMINI.md` (Engineering Mandates).
- [x] `design.md` (System Design & Visuals).
- [x] `CONTEXT.md` (Ubiquitous Language).
- [ ] `CONTRIBUTING.md`.
- [ ] `.github/workflows/ci.yml`.
- [ ] Comprehensive Integration Tests.
