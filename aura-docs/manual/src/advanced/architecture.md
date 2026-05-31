# System Architecture

Aura is built on a highly modular, actor-based architecture using the **Tokio** runtime. This design ensures that disk I/O, network traffic, and UI updates never block each other.

## Core Actors

### 1. The Orchestrator
The "brain" of Aura. It is the single source of truth for the entire engine.
- **Task Management**: Spawns and kills `ProtocolWorkers`.
- **Bandwidth Control**: Coordinates with the `Throttler` to enforce limits.
- **Chain Orchestrator**: Handles task-to-task dependencies and auto-starts (e.g., HTTP -> BitTorrent handover).
- **Mapping Engine**: Resolves logical file structures to physical disk paths using metadata rules.
- **Scaling**: Analyzes EWMA metrics to scale connections.
- **Event Bus**: Broadcasts state changes (JSON-formatted) to the RPC server, WebUI, and TUI.

### 2. The Storage Engine
A centralized actor responsible for all disk interactions.
- **Sequential Write Aggregation**: Reorders out-of-order blocks (common in BitTorrent) in RAM before performing large, sequential disk writes.
- **Atomic Operations**: Manages `.part` files and renames them only after successful hash verification and `fsync`.
- **Buffer Pool**: Reuses memory-aligned buffers to minimize heap allocations and CPU cache misses.
- **No-COW Aware**: Automatically disables Copy-on-Write for Btrfs/ZFS to maintain high random-write performance.

### 3. Protocol Workers
Lightweight, disposable actors spawned for each download source.
- **Stateless Logic**: Workers are isolated; they only know about their assigned ranges and the central Orchestrator.
- **Adaptive Racing**: Workers monitor their own latency and report it to the Orchestrator, which may trigger "Work Stealing" if they lag.

## Data Flow (The Green Path)

1.  **Request**: User adds a URI via CLI, RPC, or Browser Bridge.
2.  **Maturity**: Orchestrator spawns a worker to resolve metadata (size, name, info-hash).
3.  **Planning**: Once matured, the Orchestrator generates a range-map based on available sources.
4.  **Fetching**: Workers request tokens from the `Throttler`, fetch data, and stream it to the `Storage Engine`.
5.  **Commit**: `Storage Engine` aggregates writes, verifies hashes (Merkle or linear), and signals completion.

## Concurrency & Backpressure

Aura uses **Bounded MPSC Channels** for all inter-actor communication.
- **Disk Backpressure**: If the disk I/O subsystem is saturated, the `Storage Engine`'s input channel will fill up.
- **Natural Throttling**: Because channels are bounded, protocol workers will automatically block when trying to send data to a full storage queue. This naturally slows down network ingestion, preventing memory exhaustion (OOM) during high-speed downloads on slow disks.
