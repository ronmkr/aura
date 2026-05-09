# Aura: Project Learnings & Analysis

## Overview
`Aura` is the high-performance, asynchronous Rust successor to `aria2`. It utilizes an actor-based architecture to provide a safer, more concurrent, and more maintainable download orchestration engine.

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

### Milestone 6: Persistence & Advanced Protocols (In Progress)
- **Task Persistence Architecture**: Saving task state (`.aura` files) allows for session recovery. The most complex part is reconstructing the **BitTorrent Piece Picker** and **Global Bitfield** from disk to avoid re-downloading validated data.
- **NAT Traversal Resilience**: Production-grade BitTorrent requires reachability. Implementing a dual-stack fallback (UPnP via `igd-next` and NAT-PMP/PCP via `crab_nat`) ensures the client is connectable in most consumer network environments. Discovery of the gateway IP is a prerequisite; while UPnP handles this internally, NAT-PMP/PCP often requires manual gateway detection or assuming the `.1` address in the local subnet.
