# Aura Gap Analysis & Validation Report

## Executive Summary
This report analyzes the gaps between the Aura codebase implementation, documentation, design decisions, and industry-standard benchmarks for high-performance download engines and BitTorrent clients (like `aria2`, `wget`, and `qBittorrent`).

During this audit and remediation cycle:
1. **Critical Feature Gap Resolved**: Implemented full support for **BitTorrent Private Torrents (BEP 27)**. Previously, the parser and connection loops ignored the `private` flag, risking user security and tracker bans.
2. **Quality Verification**: Ran the complete quality check suite (`make green-loop`). All unit tests, integration tests, Clippy lints, formatting, and file line limit constraints (modularity) pass with **zero warnings and zero errors**.

---

## 1. Industry Standard Feature Matrix
Comparison of Aura against standard tools (`aria2`, `wget`, `qBittorrent`):

| Feature | aria2 | wget | qBittorrent | Aura Status |
| :--- | :--- | :--- | :--- | :--- |
| **Multi-protocol engine** | Yes | Yes (HTTP/FTP) | No (BT only) | **Yes** (HTTP, HTTPS, FTP, FTPS, NNTP, S3, GDrive, BT) |
| **BitTorrent Private Flag (BEP 27)** | Yes | N/A | Yes | **Yes** (Implemented in this session) |
| **BitTorrent v2 (hybrid & Merkle)** | No | N/A | Yes | **Yes** (Implemented) |
| **UTP / LEDBAT Transport** | Yes | N/A | Yes | **Yes** (Implemented) |
| **Desktop Notifications** | No | No | Yes | **Yes** (Implemented, configurable) |
| **Watch Folder Auto-Ingest** | No | No | Yes | **Yes** (Implemented) |
| **HSTS & Alt-Svc Caching** | Yes | No | N/A | **Yes** (Implemented) |

---

## 2. Resolved Gap: Private Torrents (BEP 27)
*   **The Issue**: Private trackers mandate that clients disable public peer discovery mechanisms (DHT, PEX, and Local Peer Discovery) for torrents flagged as private. Exposing peer information for these torrents violates tracker rules and leads to user account suspension.
*   **The Solution**: 
    1.  **Metadata Parsing**: Updated `Info` struct in `aura-core/src/torrent/metadata.rs` to parse the optional `private` bencode field.
    2.  **Privacy Utility**: Implemented `is_private()` on the `Torrent` struct in `aura-core/src/torrent/logic.rs`.
    3.  **DHT Suppression**: Updated the worker's DHT loop (`aura-core/src/worker/bittorrent/task/dht.rs`) to break early and prevent peer lookup if the torrent is private.
    4.  **LPD Suppression**: Updated the Local Peer Discovery loop (`aura-core/src/worker/bittorrent/task/lpd.rs`) to exit and send a `Remove` command if the torrent is private.
    5.  **PEX Suppression**: Prevented peer exchange negotiation in handshakes and ignored incoming/outgoing PEX updates in `aura-core/src/worker/bittorrent/worker/loop_logic.rs` and `handlers/incoming.rs`.
    6.  **Tracker/Command Verification**: Updated command refresh actions to skip announcing private torrents.
    7.  **Unit Tests**: Added a dedicated `test_private_torrent` unit test in `aura-core/src/torrent/tests.rs` to guarantee correct serialization, parsing, and property check behavior.

---

## 3. Remaining Documented Gaps
The following gaps exist between the documentation/ADRs and the current implementation:

### 🔴 P0 — Critical Gaps
*   **GAP-P0-1: Hardcoded Relative Paths for HSTS and Alt-Svc Cache**
    *   *Problem*: HSTS and Alt-Svc caches are saved relative to the current working directory (`.aura/hsts.json`). They should derive their paths from the dynamic sandbox root configuration.
*   **GAP-P0-2: Stale FAQ NNTP Stub Claim**
    *   *Problem*: `faq.md` claims NNTP is a stub. NNTP is actually fully implemented.

### 🟠 P1 — High-Impact Gaps
*   **GAP-P1-1: Stale ROADMAP Milestone Future Backlog**
    *   *Problem*: `ROADMAP.md` backlog lists HTTP/3 QUIC, Task Chaining, NFS optimizations, and Mirroring as missing. They are fully implemented.
*   **GAP-P1-2: Disk I/O Scheduling marked "Partial"**
    *   *Problem*: ADR-0022 index claims "Partial" but doesn't detail what remains (e.g., `io_uring` integration).
*   **GAP-P1-3: NNTP Missing Standalone ADR**
    *   *Problem*: NNTP is bundled as a footnote in ADR-0023 instead of having a dedicated ADR.
*   **GAP-P1-4: Undocumented `[bulk]` Config Section**
    *   *Problem*: `max_scan_depth` setting is undocumented.

---

## 4. Quality Validation Status
*   **Modularity Constraints**: All source files strictly adhere to the 350-line size limit (e.g. `mod.rs` compacted to 322 lines).
*   **Compiler Warnings**: Clippy checks compile clean with **zero warnings** under strict `-D warnings`.
*   **Unit & Integration Tests**: 100% of the test suite passes successfully.
