# Process Resilience & Crash Recovery

Aura is built to operate reliably as a headless system service. It includes mechanisms to recover from worker panics, manage system resources like file descriptors, and generate diagnostic reports when fatal exceptions occur.

## 1. Overview (ADR 0064)

The resilience layer is designed to isolate failures. A panic in a single BitTorrent peer worker or HTTP connection segment will not crash the daemon. Aura uses thread-isolated actor supervisions to log, restart, or failover affected tasks dynamically.

## 2. Panic Hook & Crash Log

Aura overrides the default Rust panic behavior to ensure diagnostic information is captured before thread termination:
- **Crash Log File**: Emergency backtraces, thread details, and engine states are written to `~/.aura/crash.log` (or `%AppData%\aura\crash.log` on Windows).
- **Graceful Shutdown**: The panic hook attempts to instruct the Orchestrator to flush state files (`.aura`) to disk before exiting the main process if a fatal global panic occurs.

## 3. File Descriptor (FD) Management

Concurrently downloading thousands of blocks across hundreds of BitTorrent peers requires a significant number of open network sockets and file handles. 

- **Dynamic Raising**: On startup, the daemon checks the system's file descriptor limits. If the soft limit is lower than the configured requirements, Aura attempts to dynamically raise the process soft limit to the system's maximum hard limit.
- **Backpressure**: If the system runs out of available file descriptors despite this, the Storage Engine chokes incoming connection requests and places worker creations in a retry queue to prevent crash loops.

## 4. Storage Integrity Scrubbing

In addition to runtime resilience, Aura protects against media decay (bit rot) on disk:
- **Active Scrubber**: The background Integrity Scrubber (ADR 0024) scans active downloads, validates piece hashes, and automatically re-downloads corrupted blocks from the network.
