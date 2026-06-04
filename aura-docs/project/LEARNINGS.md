# Aura: Project Learnings & Analysis

## Overview
`Aura` is the high-performance, asynchronous Rust download engine. It utilizes an actor-based architecture to provide a safer, more concurrent, and more maintainable download orchestration engine.

## Engineering Baseline
- **Language**: Rust (Edition 2021).
- **Runtime**: `tokio` (Multi-threaded async).
- **Communication**: Actor model using `mpsc` for commands and `broadcast` for telemetry.
- **I/O Engine**: Modular Storage Engine targeting `io_uring` for extreme throughput.

## Milestone Progress

### Milestone 2: The "Smart" Buffer (Completed)
- **Atomic Completion**: Writing to `.part` files and renaming only after 100% verification prevents the "partial file" corruption common in standard downloaders.
- **Sequential Aggregation**: Reordering writes in memory before flushing to disk is critical for BitTorrent performance on physical disks (HDDs).
- **Iron-Clad Networking**: Standard HTTP clients are too permissive for download managers. Implementing manual redirect management and **Content Sniffing** (identifying HTML even when the server lies about MIME types) is mandatory for robustness.
- **TDD with Wiremock**: Using `wiremock` for actor integration tests allows simulating hostile network environments (redirect loops, slow streams) that are difficult to reproduce with real servers.

### Milestone 6: Persistence & Advanced Protocols (Completed)
- **Task Persistence Architecture**: Saving task state (`.aura` files) allows for session recovery. The most complex part is reconstructing the **BitTorrent Piece Picker** and **Global Bitfield** from disk to avoid re-downloading validated data.
- **NAT Traversal Resilience**: Production-grade BitTorrent requires reachability. Implementing a dual-stack fallback (UPnP via `igd-next` and NAT-PMP/PCP via `crab_nat`) ensures the client is connectable in most consumer network environments. Discovery of the gateway IP is a prerequisite; while UPnP handles this internally, NAT-PMP/PCP often requires manual gateway detection or assuming the `.1` address in the local subnet.

### Milestone 7: Industrial Hardening (In Progress)
- **Kernel TLS (kTLS)**: Offloading TLS symmetric decryption directly to kernel-space enables a true zero-copy pipe from the NIC to disk via page-splicing (`sendfile`), circumventing user-space memory copies and making 10Gbps+ links highly CPU-efficient.
- **Starbucks WiFi & Captive Portals**: Redirected HTTP requests are a silent corruption vector. Programmatic validation of landing-page/login HTML bodies during large binary downloads protects files from being overwritten with HTML login pages.
- **Interface Roaming**: Interface changes (e.g., undocking a laptop or moving to a different Wi-Fi network) are best handled by monitoring OS netlink routing events, automatically pausing workers, and seamlessly rebuilding sockets on the new default interface without failing the active task.
- **Resource-Aware Multi-Tenancy**: Supporting multiple users on a single daemon demands robust `TenantContext` isolation. Wiring independent speed limits (`Throttler`), task bounds, and sandbox directory roots directly into orchestrator events guarantees multi-user security.
- **Observability Spans**: Structured trace logging (`tracing-subscriber` JSON formatting) is essential for asynchronous actor-based systems where standard logs become an unreadable mix of concurrent events.
- **Hierarchical Configuration & CLI Overrides**: Merging file resolution paths (direct path, working directory, user configurations) and applying runtime overrides via CLI flags ensures consistent settings across core, daemon, and CLI modules without hardcoded values.
- **Policy-Based Error Classification & Self-Healing**: Decoupling inline error logic into a modular `PolicyManager` categorizes failures into worker, task, and engine scopes, facilitating automatic retries with exponential backoff for transient mirror degradations.
