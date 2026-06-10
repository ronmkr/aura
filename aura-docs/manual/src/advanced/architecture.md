# System Architecture

Aura is built on a highly modular, actor-based architecture using the **Tokio** runtime. This design ensures that disk I/O, network traffic, and UI updates never block each other.

## Core Actors

### 1. The Orchestrator
The "brain" of Aura. It is the single source of truth for the entire engine.
- **Task Management**: Spawns and kills `ProtocolWorkers`.
- **Protocol Detection (ADR 0065)**: Uses a centralized `ProtocolDetector` to automatically infer task types (HTTP, FTP, BitTorrent, Metalink) from URIs, local paths, or Info-Hashes.
- **Bandwidth Control**: Coordinates with the `Throttler` to enforce static and scheduled limits (ADR 0063).
- **Chain Orchestrator**: Handles task-to-task dependencies and auto-starts (e.g., HTTP -> BitTorrent handover).
- **Mapping Engine**: Resolves logical file structures to physical disk paths using metadata rules.
- **Scaling**: Analyzes EWMA metrics to scale connections.
- **Event Bus**: Broadcasts state changes (JSON-formatted) to the RPC server, WebUI, and TUI.

### 2. The Storage Engine
A centralized actor responsible for all disk interactions.
- **Sequential Write Aggregation (ADR 0033)**: Reorders out-of-order blocks in RAM before performing large, sequential disk writes. It prefers flushing contiguous blocks to disk to reduce fragmentation.
- **Generation-based Writes (ADR 0033)**: Every piece request is assigned a unique Generation ID. The Storage Engine only commits writes that match the current active generation, ensuring that slow "losers" of a Work Stealer race cannot overwrite fresher data.
- **Disk I/O Scheduler (ADR 0022)**: Uses kernel hints and asynchronous I/O to maximize throughput.
    - **Kernel Hinting**: Aura applies `POSIX_FADV_SEQUENTIAL` for single-stream downloads and `POSIX_FADV_DONTNEED` after verification to keep the OS page cache clean.
    - **io-uring Integration**: On supported Linux systems, Aura utilizes `io-uring` for zero-copy, non-blocking writes.
- **Atomic Operations**: Manages `.part` files and renames them only after successful hash verification and `fsync`.
- **Disk Space Verification (ADR 0060)**: Queries filesystem availability before pre-allocation and enforces safety headroom (5% or 512MB).
- **Zero-Copy Intent**: Uses `BytesMut` for efficient memory management without pooling overhead.
- **No-COW Aware**: Automatically disables Copy-on-Write for Btrfs/ZFS.

### 3. Protocol Workers
Lightweight, disposable actors spawned for each download source.
- **Stateless Logic**: Workers are isolated and easily replaceable.
- **Fine-Grained Cancellation**: Orchestrator instantly aborts workers that lose a "Work Stealing" race.
- **Selective Downloading (ADR 0065)**: Storage and workers coordinate to skip pieces for unselected files in a swarm while correctly handling shared boundary pieces.

---

## Process Resilience & Safety (ADR 0064)

Aura is designed for mission-critical stability:
- **Panic Hook**: A global `std::panic::set_hook` captures backtraces and saves them to `~/.aura/crash.log` before emergency shutdown.
- **Task Recovery**: Critical tasks (Orchestrator, Storage) are monitored via `JoinHandle`. On panic, the engine attempts an emergency state flush to disk before exiting.
- **FD Management**: Aura automatically calculates and attempts to raise the OS file descriptor limit (`RLIMIT_NOFILE`) to prevent connection drops.

---

## Interactive TUI Architecture (ADR 0065)

The **Pilot Dashboard** (TUI) uses a stateful **ViewRouter** architecture:
- **Enum State Machine**: Manages a navigation stack (Dashboard -> Mission Control -> File Selector).
- **Reactive Rendering**: The UI only redraws affected areas, maintaining 60fps even with thousands of files in a virtualized tree view.
- **Buffered Telemetry**: Throughput history is maintained in the TUI process using circular buffers to power the real-time sparkline charts.

---

## Persistence & History (ADR 0062)

- **Control Files**: Active task states are persisted in `~/.aura/tasks/` using JSON files for fast recovery after restarts.
- **History Log**: Completed, failed, and removed tasks are recorded in an append-only `~/.aura/history.jsonl` file. This log is rotated automatically when it reaches 10MB to maintain performance.

---

## Concurrency & Backpressure

Aura uses **Bounded MPSC Channels** for all inter-actor communication.
- **Disk Backpressure**: If the disk I/O subsystem is saturated, the `Storage Engine`'s input channel fills up.
- **Natural Throttling**: Protocol workers automatically block when trying to send data to a full storage queue, naturally slowing down network ingestion and preventing memory exhaustion.
