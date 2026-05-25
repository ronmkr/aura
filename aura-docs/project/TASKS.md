# Aura: Development Tasks (Post-Deep-Dive Audit)

This document tracks the technical debt and missing features identified during the 2026-05-24 code-level deep-dive audit across all 4 crates. Updated from the earlier 2026-05-09/2026-05-17 audits with corrections.

## 🔴 Critical: Data Integrity & Security

- [x] **fsync before atomic rename** (Issue #120) [CRITICAL]
    - `storage/ops.rs` does NOT call `fsync()`/`fdatasync()` before `.part` → final rename.
    - Crash before OS writeback could cause silent data loss/corruption.
- [ ] **DHT token is hardcoded `[1,2,3,4]`** (Issue #121) [CRITICAL]
    - `dht/actor/` uses a predictable static token for all announce_peer responses.
    - Remote peers can forge announcements — security vulnerability per BEP 5.
- [ ] **VPN kill-switch is dead code** (Issue #122) [HIGH]
    - `force_tunnel` config key exists but is never read/enforced in `vpn/logic.rs`.
    - No firewall rules, no iptables/pf integration, no automatic interface binding.
    - Users relying on `force_tunnel = true` have a false sense of security.
- [x] **DNS resolver config is a facade** (Issue #128) [MODERATE]
    - `create_resolver()` accepts Cloudflare/Google/Custom enum variants but ALL paths create the same system resolver.
    - `hickory-resolver` with DoH feature is a dependency but never configured for DoH/DoT.

## 🟡 High Priority: Protocol Correctness

- [ ] **BT choking algorithm** (Issue #123) [HIGH]
    - Worker tracks `peer_choking` state but NEVER sends outgoing Choke/Unchoke/NotInterested.
    - No tit-for-tat, no optimistic unchoke cycle, no upload bandwidth management.
    - `peer_registry` `am_choking`/`am_interested` fields initialized but never updated.
- [ ] **PEX (Peer Exchange) — BEP 11** (Issue #124) [MODERATE]
    - Config flag `pex_enabled` exists but zero PEX code anywhere in the codebase.
    - README and ROADMAP list PEX as implemented — this is inaccurate.
- [ ] **Peer registry health scoring** (Issue #126) [MODERATE]
    - `peer_registry/logic.rs` (115 lines) only tracks connection state.
    - No speed tracking, latency, error/corrupt piece counting, reputation, banning, or eviction.
    - Peer selection is naive: first Disconnected peer, no prioritization.
- [ ] **FTP: No FTPS (TLS) and no retry** (Issue #125) [MODERATE]
    - `into_secure()` never called — all FTP connections plaintext.
    - No retry logic — single attempt, any error is terminal.
- [ ] **BT block size is 32KB** (Issue #131) [LOW]
    - Non-standard (spec says 16KB). May cause compatibility issues with strict peers.
- [ ] **Tracker tier ordering** (Issue #132) [LOW]
    - All trackers queried in parallel, not by BEP 12 tier priority.

## 🟡 Medium Priority: Correctness & UX

- [ ] **BDD stub test implementations** (Issue #127) [MODERATE]
    - 41 empty step functions across 4 feature files (daemon, networking, storage, swarm).
    - 9 scenarios compile and pass vacuously but test nothing.
- [ ] **Daemon ignores rpc_port config** (Issue #129) [LOW]
    - Hardcoded `0.0.0.0:6800`, ignores `config.network.rpc_port`.
- [ ] **Metalink priority hardcoded + debug eprintln** (Issue #130) [LOW]
    - Priority always 0 (not parsed from XML `<url>` attribute).
    - `eprintln!("DEBUG: ...")` left in production code at `metalink/logic.rs:83`.
- [ ] **QR Code Sharing** (Issue #74) [MINOR]
- [ ] **i18n Support for CLI and TUI** (Issue #71) [MINOR]

## 🟢 Low Priority: Future Features (Milestone 7+)

- [ ] **Advanced Disk I/O Scheduling** (Issue #13) — Deadline-based sorted writes.
- [ ] **Generational Write Buffer** (Issue #14) — Current aggregator is a simple BTreeMap with no eviction.
- [ ] **Multi-tenancy & Quotas** (Issue #15) — No tenant isolation or resource quotas.
- [ ] **Integrity Scrubbing & Self-healing Storage** (Issue #4) — No periodic re-verification.
- [ ] **Task Chaining & Dependencies** (Issue #11) — No DAG-based scheduler.
- [ ] **Recursive Mirroring** (Issue #65) — No wget-style site crawling.
- [ ] **Network Filesystem (NFS/SMB) Optimizations** (Issue #12) — No FS type detection beyond Btrfs/ZFS.
- [ ] **Task Priority Scheduling** (Issue #72) — `priority` field exists in API but orchestrator doesn't sort by it.
- [ ] **Prioritized Streaming Mode** (Issue #28) — Sequential piece picker exists but no seek-based scheduling.
- [ ] **HTTP/3 QUIC** (Issue #23).
- [ ] **HSTS Cache** (Issue #20).
- [ ] **Advanced Networking (kTLS, Captive Portals)** (Issue #8).
- [ ] **New Protocols** (NNTP #22, Cloud #10).
- [ ] **CI Docs Deployment**.

## ✅ Previously Completed (Verified by Deep-Dive)

- [x] **Advanced Filesystem Hardening** (Issue #92) — No-COW on Btrfs/ZFS, fallocate, path hardening.
- [x] **Modular Architecture Refactor** (Issue #96) — No file >400 lines (except http.rs at 625).
- [x] **Project Documentation** — DEVELOPMENT.md, API.md, MIGRATION.md, 18-page mdBook manual.
- [x] **Policy-based Error Management** (Issue #18) — Retry backoff, mirror failover, blacklisting.
- [x] **Browser Extension Bridge** (Issue #95) — `/extension/add` endpoint.
- [x] **WebSocket Telemetry for RPC** — Real-time `aura.onEvent` streaming.
- [x] **Web UI Dashboard** (Issue #67) — Real SPA with Catppuccin dark theme.
- [x] **Prometheus Metrics Exporter** (Issue #69) — `/metrics` endpoint.
- [x] **Integrated Hook System** (Issue #16) — 4 event types, script execution.
- [x] **MIME Validation & Landing Page Resolution** (Issue #19) — HTML scraping with asset link matching.
- [x] **Unified Credential Provider** (Issue #21) — Netrc + Netscape cookie parsing.
- [x] **Dynamic DHT Bootstrap** (Issue #77) — sled persistence, periodic re-pinging.
- [x] **Non-Swarm Integrity** (Issue #75) — MD5/SHA-1/SHA-256/SHA-512 at storage layer.
- [x] **SOCKS5 Proxy Support** (Issue #66) — `reqwest::Proxy` + `tokio_socks`.
- [x] **NAT Traversal & Port Mapping Refresh** (Issue #114) — 30-minute refresh interval.
- [x] **Happy Eyeballs** (Issue #25) — IPv6/IPv4 interleaving with 250ms stagger.
- [x] **VPN Detection** (Issue #42) — WireGuard, OpenVPN, InterfaceMonitor providers (but kill-switch not enforced).
