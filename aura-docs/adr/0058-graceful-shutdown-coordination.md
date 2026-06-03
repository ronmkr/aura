# ADR 0058: Graceful Shutdown Coordination

## Status
Implemented (2026-06-03, remediation/immediate-security)

## Context
Aura is an asynchronous multi-actor system. When the user signals termination (e.g., via `Ctrl+C` or a SIGTERM signal), the daemon and CLI processes must exit cleanly. Currently, no signal handling is implemented in the CLI or daemon main functions (GAP-37), causing immediate process exit. This abrupt termination leaves active `.part` files in an inconsistent/corrupted state, fails to serialize and persist DHT routing tables (ADR 0017), fails to announce final stop events to trackers, and does not cleanly close TCP sockets.

## Decision
1. **Signal Trapping Layer**: Implement a listener using `tokio::signal` in both `aura-daemon` and `aura-cli` entry points.
2. **Shutdown Coordinator**: Introduce a structured shutdown coordinator in the Orchestrator. When a signal is caught:
   - Transition to `ShuttingDown` status.
   - Stop accepting new tasks or piece allocations.
   - Trigger flush of all dirty cache blocks and write queues in the Storage Engine.
   - Instruct discovery actors (DHT, Tracker) to serialize their state (e.g., writing the routing table to `dht.dat`).
   - Send `Stopped` event announcements to all active trackers/peers.
3. **Grace Period**: Configure a maximum shutdown timeout (e.g., 5 seconds). If the actors do not exit within this window, force terminate the process to prevent hanging.

## Edge Cases
1. **Double Signal Interrupt**: If a user hits `Ctrl+C` twice, it indicates they want an immediate exit. The coordinator must intercept a second interrupt and trigger an immediate `std::process::exit(130)` without waiting for flushes.
2. **Stuck I/O and Dead NAS mounts**: If flushing a dirty write-back buffer is blocked because a network share (NFS/SMB) went offline, the shutdown could hang indefinitely. The shutdown grace timeout (e.g., 5s) must be enforced via `tokio::time::timeout` and abort the tokio runtime if expired.
3. **In-Flight Tracker Announcements**: Sending a `Stopped` event to a tracker requires a network request. If DNS resolution or the tracker server is unresponsive, the network timeout must be capped at a small limit (e.g., 2 seconds) so it does not block the shutdown sequence.
4. **Orphan Thread Join**: Any OS threads spawned outside the Tokio runtime (e.g. specialized system calls or CPU hashing threads) must be configured as daemon threads or signaled via atomic flags to exit, preventing them from blocking process termination.

## Alternatives Considered
- **Immediate Hard Exit**: Let the OS clean up sockets and file descriptors. *Rejected:* Sockets are closed, but data buffers in memory are lost, leading to file fragmentation and partial write corruption, requiring expensive re-hashing on startup.
- **Synchronous Dropping**: Relying on Rust's `Drop` implementation to flush state. *Rejected:* Asynchronous operations (like writing to disk or sending network tracker packets) cannot be safely block-waited in standard sync `Drop` implementations without running into tokio runtime panics.

## Consequences
- **Pros**: Clean shutdowns; avoids file corruption; preserves peer discovery caches; guarantees proper protocol announcements.
- **Cons**: Minor startup initialization code; adds exit delay during shutdown phase.
