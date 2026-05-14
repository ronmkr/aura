# Aura: Development Tasks (Post-Audit Gaps)

This document tracks the technical debt and missing features identified during the Milestone 1-6 audit on 2026-05-09.

## 🔴 High Priority: Stability & Performance
- [x] **Advanced Filesystem Hardening** (Issue #92)
    - Implement `fallocate` for actual block allocation (Linux).
    - Implement COW-awareness (`chattr +C`) for Btrfs/ZFS.
    - Implement Windows long path prefixing (`\\?\`).
- [x] **Modular Architecture Refactor** (Issue #96)
    - Decompose `aura-core/src/torrent/` (was 461 lines) into sub-modules.
    - Decompose `aura-core/src/dht/` (was 445 lines) into sub-modules.

## 🟡 Medium Priority: Connectivity & UX
- [x] **Browser Extension Bridge** (Issue #95)
    - Implement RPC gateway for Chrome/Firefox extensions.
    - Add protocol interceptors for magnet/torrent/metalink MIME types.
- [x] **WebSocket Telemetry for RPC** (Edge Case from ADR 0016)
    - Implement WebSocket support in `aura-daemon` for real-time `EventBus` subscriptions.
    - Support bi-directional streaming for frontend (TUI/Web) parity.
- [x] **Native VPN Integration** (Issue #42)
    - Implement `WireGuard` controller (CLI/IPC).
    - Implement `OpenVPN` controller (Management Interface).
    - Add "Mandatory Tunnel" enforcement.
- [ ] **Web UI Dashboard** (Issue #67)
    - Built-in static file server in the Daemon.
    - Full parity with CLI/TUI functionality.
- [ ] **QR Code Sharing** (Issue #74)
    - Generate QR codes for magnet links in CLI/TUI.
- [ ] **i18n Support for CLI and TUI** (Issue #71)
    - Externalized string resources and multiple language support.
- [ ] **Prometheus Metrics Exporter** (Issue #69)
    - Implement telemetry endpoint for monitoring tools.
- [ ] **Integrated Hook System** (Issue #16)
    - Event callbacks for task completion/error via external scripts.

## 🟢 Low Priority: Maintenance & Advanced Features
- [ ] **Dynamic DHT Bootstrap** (Issue #77)
    - Implement periodic re-pinging and high-uptime node persistence.
- [ ] **Non-Swarm Integrity** (Issue #75)
    - Support `--checksum` for HTTP and FTP tasks.
- [ ] **Privacy-Enhanced Resolver (DoH/DoT)** (Issue #73)
    - Async DNS via Cloudflare/Google over HTTPS.
- [ ] **Task Prioritization & Dependencies** (Issue #72)
    - Manage download order and chained dependencies (Issue #11).
- [ ] **SOCKS5 Proxy Support** (Issue #66)
    - Complete proxy routing for BitTorrent and HTTP protocols.
- [ ] **Prioritized Streaming Mode** (Issue #28)
    - Piece selection optimized for media streaming.
- [ ] **Modern Networking & Fallbacks**
    - Happy Eyeballs (Issue #25), HTTP/3 QUIC (Issue #23), HSTS Cache (Issue #20).
- [ ] **New Protocols**
    - NNTP Support (Issue #22), Cloud Storage integration (Issue #10).
- [ ] **CI Docs Deployment**
    - Add a GitHub Action to deploy `aura-docs/manual` to GitHub Pages.
