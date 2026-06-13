# Architecture Decision Records (ADR)

Aura's design is driven by a series of formal Architecture Decision Records. These documents explain the *why* behind every major technical choice.

| ADR | Title | Status |
|---|---|---|
| [0001](../adr/0001-orchestrated-pull-model.md) | ADR 0001: Orchestrated Pull Model for Work Assignment | Implemented (2026-05-06, commit 0777b1ab) |
| [0002](../adr/0002-centralized-storage-writing.md) | ADR 0002: Centralized Storage Writing and Ownership | Implemented (2026-05-06, commit 0777b1ab) |
| [0003](../adr/0003-atomic-completion-and-pre-allocation.md) | ADR 0003: Atomic Completion and Pre-allocation Strategy | Implemented (2026-05-27, PR #99) |
| [0004](../adr/0004-telemetry-and-event-bus.md) | ADR 0004: Telemetry and Event Bus Architecture | Implemented (2026-05-06, commit 0777b1ab) |
| [0005](../adr/0005-racing-work-stealer.md) | ADR 0005: Racing Work Stealer for Slow Stream Mitigation | Implemented (2026-05-06, commit 0777b1ab) |
| [0006](../adr/0006-error-classification-and-self-healing.md) | ADR 0006: Error Classification and Self-healing Strategy | Implemented (2026-06-04, PR #221) |
| [0007](../adr/0007-protocol-encapsulation.md) | ADR 0007: Protocol Encapsulation and Black-Box Workers | Implemented (2026-05-06, commit 0777b1ab) |
| [0008](../adr/0008-lifecycle-based-task-maturation.md) | ADR 0008: Lifecycle-based Task Maturation (Magnet Links) | Implemented (2026-05-06, commit 0777b1ab) |
| [0009](../adr/0009-global-token-bucket-throttling.md) | ADR 0009: Global Token Bucket Throttling | Implemented (2026-05-06, commit 0777b1ab) |
| [0010](../adr/0010-decoupled-peer-discovery.md) | ADR 0010: Decoupled Peer Discovery and Registry | Implemented (2026-05-06, commit 0777b1ab) |
| [0011](../adr/0011-dynamic-configuration.md) | ADR 0011: Dynamic Configuration and Hot-reloading | Implemented (2026-05-06, commit 0777b1ab) |
| [0012](../adr/0012-tui-theming-and-proxy.md) | ADR 0012: Themeable TUI and Proxy Connector | Implemented (2026-05-06, commit 0777b1ab) |
| [0013](../adr/0013-cloud-and-metalink.md) | ADR 0013: Cloud Storage and Metalink Integration | Implemented (2026-06-11, Issue #10) |
| [0014](../adr/0014-credential-and-security.md) | ADR 0014: Credential and Security Abstraction | Implemented (2026-05-06, commit 0777b1ab) |
| [0015](../adr/0015-url-globbing.md) | ADR 0015: URL Globbing and Batch Processing | Implemented (2026-05-06, commit 0777b1ab) |
| [0016](../adr/0016-rpc-and-interface.md) | ADR 0016: RPC Server and Interface Binding | Implemented (2026-06-04, PR #261) |
| [0017](../adr/0017-segmentation-and-persistence.md) | ADR 0017: Segmentation and Discovery Persistence | Implemented (2026-06-04, PR #259) |
| [0018](../adr/0018-hooks-hsts-ftp.md) | ADR 0018: Hooks, HSTS, and Multi-Channel Protocols | Implemented (2026-05-06, commit 0777b1ab) |
| [0019](../adr/0019-buffer-pool-and-caching.md) | ADR 0019: Buffer Pool and Write-Back Caching | Superceded (by Issue #160, 2026-05-30, PR #164) |
| [0020](../adr/0020-engine-api.md) | ADR 0020: Engine API and Library Embeddability | Implemented (2026-05-06, commit 0777b1ab) |
| [0021](../adr/0021-network-filesystem-optimization.md) | ADR 0021: Network Filesystem Optimization (NFS/SMB) | Implemented (2026-06-04) |
| [0022](../adr/0022-disk-io-scheduling.md) | ADR 0022: Advanced Disk I/O Scheduling and Kernel Hinting | Partially Implemented (Audit 2026-06-03) |
| [0023](../adr/0023-adaptive-scaling-and-aggregation.md) | ADR 0023: Adaptive Connection Scaling and Sourced Aggregation | Partially Implemented (2026-05-25, PR #90) |
| [0024](../adr/0024-integrity-scrubbing.md) | ADR 0024: Integrity Scrubbing and Torrent Refreshing | Implemented (2026-05-06, commit 0777b1ab) |
| [0025](../adr/0025-nat-traversal-and-lpd.md) | ADR 0025: NAT Traversal and LAN Discovery | Implemented (2026-05-27, PR #114) |
| [0026](../adr/0026-modern-networking.md) | ADR 0026: Modern Networking (Happy Eyeballs, Alt-Svc, Streaming) | Implemented (2026-06-10, PR #277) |
| [0027](../adr/0027-power-management.md) | ADR 0027: Power Management and Automated Lifecycle Actions | Implemented (2026-05-06, commit 0777b1ab) |
| [0028](../adr/0028-privacy-dns.md) | ADR 0028: Privacy-Enhanced Resolution and Modern DNS | Implemented (2026-05-27, PR #116) |
| [0029](../adr/0029-mapping-and-chaining.md) | ADR 0029: Resource Mapping and Task Chaining | Implemented (2026-05-06, commit 0777b1ab) |
| [0030](../adr/0030-recursive-mirroring.md) | ADR 0030: Recursive Mirroring and HTML Parsing | Implemented (2026-05-27, PR #142) |
| [0031](../adr/0031-bittorrent-v2-merkle.md) | ADR 0031: BitTorrent v2 and Merkle Tree Management | Implemented (2026-06-01, PR #193) |
| [0032](../adr/0032-multi-tenancy-and-tracing.md) | ADR 0032: Multi-Tenancy and Observability | Implemented (2026-05-31, PR #179) |
| [0033](../adr/0033-generation-writes-and-aggregation.md) | ADR 0033: Generation-based Writes and Sequential Aggregation | Implemented (2026-06-03) |
| [0034](../adr/0034-advanced-network-edge-cases.md) | ADR 0034: Advanced Network Edge Cases (kTLS, Roaming, Captive Portals) | Implemented (2026-05-31, PR #178) |
| [0035](../adr/0035-advanced-filesystem-edge-cases.md) | ADR 0035: Advanced Filesystem Edge Cases (COW, Long Paths, Endgame) | Implemented (2026-05-27, PR #99) |
| [0036](../adr/0036-bittorrent-core.md) | ADR 0036: BitTorrent Core and Swarm Management | Implemented (2026-05-06, commit 0777b1ab) |
| [0037](../adr/0037-redirects-and-validation.md) | ADR 0037: Redirect Handling and Content Validation | Implemented (2026-05-06, commit 0777b1ab) |
| [0038](../adr/0038-vpn-integration.md) | ADR 0038: Native VPN Integration (OpenVPN, WireGuard) | Implemented (2026-06-02, PR #194) |
| [0039](../adr/0039-bittorrent-endgame-mode.md) | ADR 0039: BitTorrent Endgame Mode | Implemented (2026-05-25, PR #82) |
| [0040](../adr/0040-task-prioritization.md) | ADR 0040: Task Prioritization and Dependency Chains | Implemented (2026-06-03, PR #199) |
| [0041](../adr/0041-non-swarm-integrity.md) | ADR 0041: Integrity Verification for Non-Swarm Protocols | Implemented (2026-05-27, PR #112) |
| [0042](../adr/0042-i18n-architecture.md) | ADR 0042: Internationalization (i18n) Architecture | Proposed |
| [0043](../adr/0043-unified-architecture.md) | ADR 0043: Unified Architecture and CLI-Daemon-TUI Integration | Implemented (2026-05-29, PR #144) |
| [0044](../adr/0044-bittorrent-choking-algorithm.md) | ADR 0044: BitTorrent Choking Algorithm (Tit-for-Tat) | Implemented (2026-05-28, PR #137) |
| [0045](../adr/0045-peer-exchange-pex.md) | ADR 0045: Peer Exchange (PEX) Implementation (BEP 11) | Implemented (2026-05-30, PR #159) |
| [0046](../adr/0046-peer-scoring-and-eviction.md) | ADR 0046: Peer Registry Health Scoring & Eviction | Implemented (2026-05-30, PR #165) |
| [0047](../adr/0047-automated-release-pipeline.md) | ADR 0047: Automated Release Pipeline | Implemented (2026-05-29, PR #144) |
| [0048](../adr/0048-ftps-tls-support.md) | ADR 0048: FTPS (TLS) Support and Retry Logic | Implemented (2026-05-28, PR #133) |
| [0049](../adr/0049-browser-bridge.md) | ADR 0049: Browser Bridge (Extension Support) | Partially Implemented — daemon bridge done (PR #102); Chrome extension pending (Issue #230) |
| [0050](../adr/0050-integration-tests-suite.md) | ADR 0050: Integration Tests Suite | Implemented (2026-05-25, PR #93) |
| [0051](../adr/0051-docker-containerization.md) | ADR 0051: Docker Containerization | Implemented (2026-05-29, PR #144) |
| [0052](../adr/0052-allocation-prober.md) | ADR 0052: Allocation Prober Diagnostic Tool | Implemented (2026-06-02, PR #196) |
| [0053](../adr/0053-bep12-tracker-tiers.md) | ADR 0053: BEP 12 Multitracker Compliance | Implemented (2026-06-02, PR #197) |
| [0054](../adr/0054-sandbox-root-confinement.md) | ADR 0054: SandboxRoot Confinement for Storage Engine | Implemented (2026-06-03, PR #215) |
| [0055](../adr/0055-secret-scrubbing-log-sanitization.md) | ADR 0055: SecretScrubber for Log Sanitization | Implemented (2026-06-03, PR #215) |
| [0056](../adr/0056-rpc-security-hardening.md) | ADR 0056: Daemon RPC Security Hardening | Implemented (2026-06-03, PR #215) |
| [0057](../adr/0057-resource-governor.md) | ADR 0057: ResourceGovernor for Global Memory Backpressure | Implemented (2026-06-03, Issue #207) |
| [0058](../adr/0058-graceful-shutdown-coordination.md) | ADR 0058: Graceful Shutdown Coordination | Implemented (2026-06-03, PR #215) |
| [0059](../adr/0059-uri-validation-ssrf-mitigation.md) | ADR 0059: URI Validation and SSRF Mitigation | Implemented (2026-06-04, PR #258) |
| [0060](../adr/0060-pre-download-disk-space-verification.md) | ADR 0060: Pre-Download Disk Space Verification | Implemented (2026-06-04, PR #259) |
| [0061](../adr/0061-http-ftp-checksum-verification.md) | ADR 0061: Checksum Verification for HTTP and FTP Downloads | Implemented (2026-06-04, PR #259) |
| [0062](../adr/0062-download-history-and-aria2-compatibility.md) | ADR 0062: Download History Log and aura Protocol Compatibility | Implemented (2026-06-04, PR #259) |
| [0063](../adr/0063-bandwidth-time-scheduling.md) | ADR 0063: Bandwidth Time Scheduling | Implemented (2026-06-04, PR #259) |
| [0064](../adr/0064-process-resilience-panic-fd-limits.md) | ADR 0064: Process Resilience — Panic Recovery, Crash Reporting, and File Descriptor Management | Implemented (2026-06-04, PRs #258, #259) |
| [0065](../adr/0065-interactive-tui-architecture.md) | ADR 0065: Interactive TUI Architecture & Selective Downloading | Implemented (2026-06-10, PR #277) |
| [0066](../adr/0066-mse-pe-encryption.md) | ADR 0066: MSE/PE Traffic Encryption | Implemented (2026-06-13, PR #301) |
| [0067](../adr/0067-utp-ledbat.md) | ADR 0067: μTP/LEDBAT Transport Layer | Proposed (2026-06-11 — Issue #286) |
| [0068](../adr/0068-fast-resume.md) | ADR 0068: Fast Resume and Piece Recheck | Proposed (2026-06-11 — Issue #284) |
| [0069](../adr/0069-watch-folder.md) | ADR 0069: Watch Folder Auto-ingestion | Proposed (2026-06-11 — Issue #288) |
| [0070](../adr/0070-rss-subscriptions.md) | ADR 0070: RSS/Atom Feed Subscriptions | Proposed (2026-06-11 — Issue #290) |
| [0071](../adr/0071-system-service.md) | ADR 0071: System Service Integration | Proposed (2026-06-11 — Issue #291) |

For a full list of ADRs, see the `aura-docs/adr/` directory in the repository.

