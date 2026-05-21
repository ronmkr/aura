# Aura: Development Tasks (Post-Audit Gaps)

This document tracks the technical debt and missing features identified during the Milestone 1-6 audit on 2026-05-09, updated after the 2026-05-17 Documentation & Stability PR.

## 🔴 High Priority: Stability & Performance
- [x] **Advanced Filesystem Hardening** (Issue #92)
- [x] **Modular Architecture Refactor** (Issue #96)
- [x] **Project Documentation & Toolchain Guide**
    - [x] Developer setup guide (`DEVELOPMENT.md`).
    - [x] Public Rust API documentation (`API.md`).
    - [x] User migration guide (`MIGRATION.md`).
- [x] **Policy-based Error Management & Self-healing** (Issue #18)
    - [x] Fixed HTTP 0-byte size assignment bug.
    - [x] Implement exponential backoff for 503/429 errors.
    - [x] Mirror failover/degradation logic.

## 🟡 Medium Priority: Connectivity & UX
- [x] **Browser Extension Bridge** (Issue #95)
- [x] **WebSocket Telemetry for RPC**
- [x] **Native VPN Integration** (Issue #42)
- [x] **Web UI Dashboard** (Issue #67)
- [ ] **QR Code Sharing** (Issue #74)
- [ ] **i18n Support for CLI and TUI** (Issue #71)
- [ ] **Prometheus Metrics Exporter** (Issue #69)
- [x] **Integrated Hook System** (Issue #16)
    - [x] Event callbacks implemented.
    - [x] Fixed synchronization bug in dynamic config reloading.
- [ ] **MIME Validation & Landing Page Resolution** (Issue #19)
    - [x] `Content-Length` fallback implemented in HTTP worker.
    - [ ] Redirect/HTML scraping for actual asset links.
- [x] **Unified Credential Provider** (Issue #21)
    - [x] .netrc parser with machine/default support.
    - [x] Netscape cookie jar support.
    - [x] Transparent injection into HTTP/FTP workers.

## 🟢 Low Priority: Maintenance & Advanced Features
- [ ] **Dynamic DHT Bootstrap** (Issue #77)
- [x] **Non-Swarm Integrity** (Issue #75)
    - [x] Support `--checksum` (MD5, SHA-1, SHA-256, SHA-512) for HTTP and FTP tasks.
    - [x] Implement `Verifying` lifecycle phase.
    - [x] Automatic part-file preservation on mismatch.
- [ ] **Privacy-Enhanced Resolver (DoH/DoT)** (Issue #73)
- [ ] **Task Prioritization & Dependencies** (Issue #72)
- [x] **SOCKS5 Proxy Support** (Issue #66)
- [ ] **NAT Traversal & Port Mapping** (Issue #25)
    - [x] Basic UPnP/NAT-PMP implementation exists.
    - [ ] Missing periodic mapping refresh logic.
- [ ] **Prioritized Streaming Mode** (Issue #28)
- [ ] **Modern Networking & Fallbacks**
    - [ ] Happy Eyeballs (Issue #25) - Genuinely missing (discrepancy found).
    - [ ] HTTP/3 QUIC (Issue #23).
    - [ ] HSTS Cache (Issue #20).
- [ ] **Advanced Networking (kTLS, Captive Portals)** (Issue #8)
- [ ] **New Protocols** (NNTP #22, Cloud #10)
- [ ] **CI Docs Deployment**
