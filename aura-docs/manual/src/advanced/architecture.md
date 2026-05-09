# System Architecture

Aura is built on a highly modular, actor-based architecture using the **Tokio** runtime. This design ensures that disk I/O, network traffic, and UI updates never block each other.

## Core Actors

### 1. The Orchestrator
The "brain" of Aura. It is the single source of truth for the entire engine.
- **Task Management**: Spawns and kills `ProtocolWorkers`.
- **Bandwidth Control**: Coordinates with the `Throttler` to enforce limits.
- **Scaling**: Analyzes EWMA metrics to scale connections.
- **Event Bus**: Broadcasts state changes to the RPC server and TUI.

### 2. The Storage Engine
A centralized actor responsible for all disk interactions.
- **Write Aggregation**: Reorders out-of-order blocks (common in BT) in RAM before performing sequential disk writes.
- **Atomic Operations**: Manages `.part` files and renames them only after successful hash verification.
- **Buffer Pool**: Reuses memory buffers to minimize heap allocations and CPU cycles.

### 3. Protocol Workers
Lightweight, disposable actors spawned for each download source.
- **Stateless**: They don't know about other mirrors; they just fetch ranges assigned by the Orchestrator.
- **Protocol-Specific**: Separate workers for HTTP (hyper), FTP (async-ftp), and BitTorrent.

## Data Flow (The Green Path)

1.  **Request**: User adds a URL via CLI or RPC.
2.  **Maturity**: Orchestrator spawns a worker to resolve metadata (size, name, info-hash).
3.  **Planning**: Once matured, the Orchestrator generates a range-map.
4.  **Fetching**: Workers request tokens from the `Throttler`, fetch data, and send it to the `Storage Engine`.
5.  **Commit**: `Storage Engine` aggregate writes, verifies hashes, and signals completion.

## Concurrency Model

Aura uses **Bounded MPSC Channels** for all inter-actor communication. This provides built-in backpressure—if the disk is slow, the storage engine's channel will fill up, which naturally slows down the network workers, preventing memory exhaustion.
