# Architecture Decision Records (ADR)

Aura's design is driven by a series of formal Architecture Decision Records. These documents explain the *why* behind every major technical choice.

| ADR | Title | Status |
|---|---|---|
| [0001](../adr/0001-orchestrated-pull-model.md) | ADR 0001: Orchestrated Pull Model for Work Assignment | Implemented |
| [0002](../adr/0002-centralized-storage-writing.md) | ADR 0002: Centralized Storage Writing and Ownership | Accepted |
| [0003](../adr/0003-atomic-completion-and-pre-allocation.md) | ADR 0003: Atomic Completion and Pre-allocation Strategy | Partially Implemented |
| [0004](../adr/0004-telemetry-and-event-bus.md) | ADR 0004: Telemetry and Event Bus Architecture | Accepted |
| [0005](../adr/0005-racing-work-stealer.md) | ADR 0005: Racing Work Stealer for Slow Stream Mitigation | Accepted |
| [0006](../adr/0006-error-classification-and-self-healing.md) | ADR 0006: Error Classification and Self-healing Strategy | Accepted |
| [0007](../adr/0007-protocol-encapsulation.md) | ADR 0007: Protocol Encapsulation and Black-Box Workers | Accepted |
| [0008](../adr/0008-lifecycle-based-task-maturation.md) | ADR 0008: Lifecycle-based Task Maturation (Magnet Links) | Accepted |
| [0009](../adr/0009-global-token-bucket-throttling.md) | ADR 0009: Global Token Bucket Throttling | Accepted |
| [0010](../adr/0010-decoupled-peer-discovery.md) | ADR 0010: Decoupled Peer Discovery and Registry | Accepted |
| [0011](../adr/0011-dynamic-configuration.md) | ADR 0011: Dynamic Configuration and Hot-reloading | Implemented |
| [0012](../adr/0012-tui-theming-and-proxy.md) | ADR 0012: Themeable TUI and Proxy Connector | Accepted |
| [0013](../adr/0013-cloud-and-metalink.md) | ADR 0013: Cloud Storage and Metalink Integration | Accepted |
| [0014](../adr/0014-credential-and-security.md) | ADR 0014: Credential and Security Abstraction | Accepted |
| [0015](../adr/0015-url-globbing.md) | ADR 0015: URL Globbing and Batch Processing | Accepted |
| [0016](../adr/0016-rpc-and-interface.md) | ADR 0016: RPC Server and Interface Binding | Accepted |
| [0017](../adr/0017-segmentation-and-persistence.md) | ADR 0017: Segmentation and Discovery Persistence | Accepted |
| [0018](../adr/0018-hooks-hsts-ftp.md) | ADR 0018: Hooks, HSTS, and Multi-Channel Protocols | Accepted |
| [0019](../adr/0019-buffer-pool-and-caching.md) | ADR 0019: Buffer Pool and Write-Back Caching | Accepted |
| [0020](../adr/0020-engine-api.md) | ADR 0020: Engine API and Library Embeddability | Accepted |
| [0021](../adr/0021-network-filesystem-optimization.md) | ADR 0021: Network Filesystem Optimization (NFS/SMB) | Accepted |
| [0022](../adr/0022-disk-io-scheduling.md) | ADR 0022: Advanced Disk I/O Scheduling and Kernel Hinting | Implemented |
| [0023](../adr/0023-adaptive-scaling-and-aggregation.md) | ADR 0023: Adaptive Connection Scaling and Sourced Aggregation | Accepted |
| [0024](../adr/0024-integrity-scrubbing.md) | ADR 0024: Integrity Scrubbing and Torrent Refreshing | Accepted |
| [0025](../adr/0025-nat-traversal-and-lpd.md) | ADR 0025: NAT Traversal and LAN Discovery | Implemented |
| [0026](../adr/0026-modern-networking.md) | ADR 0026: Modern Networking (Happy Eyeballs, Alt-Svc, Streaming) | Accepted |
| [0027](../adr/0027-power-management.md) | ADR 0027: Power Management and Automated Lifecycle Actions | Accepted |
| [0028](../adr/0028-privacy-dns.md) | ADR 0028: Privacy-Enhanced Resolution and Modern DNS | Accepted |
| [0029](../adr/0029-mapping-and-chaining.md) | ADR 0029: Resource Mapping and Task Chaining | Accepted |
| [0030](../adr/0030-recursive-mirroring.md) | ADR 0030: Recursive Mirroring and HTML Parsing | Accepted |
| [0031](../adr/0031-bittorrent-v2-merkle.md) | ADR 0031: BitTorrent v2 and Merkle Tree Management | Accepted |
| [0032](../adr/0032-multi-tenancy-and-tracing.md) | ADR 0032: Multi-Tenancy and Observability | Accepted |
| [0033](../adr/0033-generation-writes-and-aggregation.md) | ADR 0033: Generation-based Writes and Sequential Aggregation | Accepted |
| [0034](../adr/0034-advanced-network-edge-cases.md) | ADR 0034: Advanced Network Edge Cases (kTLS, Roaming, Captive Portals) | Accepted |
| [0035](../adr/0035-advanced-filesystem-edge-cases.md) | ADR 0035: Advanced Filesystem Edge Cases (COW, Long Paths, Endgame) | Partially Implemented |
| [0036](../adr/0036-bittorrent-core.md) | ADR 0036: BitTorrent Core and Swarm Management | Accepted |
| [0037](../adr/0037-redirects-and-validation.md) | ADR 0037: Redirect Handling and Content Validation | Accepted |
| [0038](../adr/0038-vpn-integration.md) | ADR 0038: Native VPN Integration (OpenVPN, WireGuard) | Partially Implemented |
| [0039](../adr/0039-bittorrent-endgame-mode.md) | ADR 0039: BitTorrent Endgame Mode | Implemented |
| [0040](../adr/0040-task-prioritization.md) | ADR 0040: Task Prioritization and Dependency Chains | Proposed |
| [0041](../adr/0041-non-swarm-integrity.md) | ADR 0041: Integrity Verification for Non-Swarm Protocols | Implemented |
| [0042](../adr/0042-i18n-architecture.md) | ADR 0042: Internationalization (i18n) Architecture | Proposed |
| [0043](../adr/0043-unified-architecture.md) | ADR 0043: Unified Architecture and CLI-Daemon-TUI Integration | Accepted |
| [0044](../adr/0044-bittorrent-choking-algorithm.md) | ADR 0044: BitTorrent Choking Algorithm (Tit-for-Tat) | Implemented |
| [0045](../adr/0045-peer-exchange-pex.md) | ADR 0045: Peer Exchange (PEX) Implementation (BEP 11) | Implemented |
| [0046](../adr/0046-peer-scoring-and-eviction.md) | ADR 0046: Peer Registry Health Scoring & Eviction | Implemented |
| [0047](../adr/0047-automated-release-pipeline.md) | ADR 0047: Automated Release Pipeline | Implemented |
| [0048](../adr/0048-ftps-tls-support.md) | 48. FTPS (TLS) Support and Retry Logic | Implemented |
| [0049](../adr/0049-browser-bridge.md) | 49. Browser Bridge (Extension Support) | Implemented |
| [0050](../adr/0050-integration-tests-suite.md) | 50. Integration Tests Suite | Implemented |
| [0051](../adr/0051-docker-containerization.md) | ADR 0051: Docker Containerization | Implemented |

For a full list of ADRs, see the `aura-docs/adr/` directory in the repository.
