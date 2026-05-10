# Aura: Development Tasks (Post-Audit Gaps)

This document tracks the technical debt and missing features identified during the Milestone 1-6 audit on 2026-05-09.

## 🔴 High Priority: Stability & Performance
- [x] **Advanced Filesystem Hardening** (Issue #92)
    - Implement `fallocate` for actual block allocation (Linux).
    - Implement COW-awareness (`chattr +C`) for Btrfs/ZFS.
    - Implement Windows long path prefixing (`\\?\`).
- [ ] **Modular Architecture Refactor** (Issue #96)
    - Decompose `aura-core/src/torrent.rs` (461 lines) into `torrent/`.
    - Decompose `aura-core/src/dht/mod.rs` (445 lines).

## 🟡 Medium Priority: Connectivity & UX
- [ ] **Browser Extension Bridge** (Issue #95)
    - Implement RPC gateway for Chrome/Firefox extensions.
    - Add protocol interceptors for magnet/torrent/metalink MIME types.
- [ ] **Native VPN Integration** (Issue #42)
    - Implement `WireGuard` controller (CLI/IPC).
    - Implement `OpenVPN` controller (Management Interface).
    - Add "Mandatory Tunnel" enforcement.

## 🟢 Low Priority: Maintenance
- [ ] **Dynamic DHT Bootstrap** (Issue #77)
    - Implement periodic re-pinging and high-uptime node persistence.
- [ ] **Non-Swarm Integrity** (Issue #75)
    - Support `--checksum` for HTTP and FTP tasks.
- [ ] **CI Docs Deployment**
    - Add a GitHub Action to deploy `aura-docs/manual` to GitHub Pages.
