# Multi-Protocol Aggregation

Aura's core strength is its ability to treat disparate sources (mirrors) as a single logical download. This process is managed by the **Sequential Aggregator** and the **Sourced Model**.

## The Sourced Model

A `MetaTask` in Aura consists of one or more subtasks. Each subtask represents a specific source (e.g., an HTTP mirror, an FTP server, or a BitTorrent swarm).

### Supported Combinations
- **HTTP + HTTP**: Speed up downloads by hitting multiple mirrors simultaneously.
- **HTTP + BitTorrent**: Use stable mirrors to "seed" a swarm or fill in missing pieces in a stalled torrent.
- **FTP + BitTorrent**: Aggregate high-speed FTP sources with P2P swarms for extreme throughput.

## Racing Work Stealer (ADR 0005)

To prevent a single slow mirror from bottlenecking the entire download, Aura implements a **Racing Work Stealer**.

### 1. Throughput Monitoring (EWMA)
The Orchestrator calculates the **Exponential Weighted Moving Average (EWMA)** throughput for every individual connection. Unlike a standard average, EWMA reacts instantly to sudden network congestion or server-side throttling.

### 2. Speculative Stealing
If a connection is significantly slower (3x slower than the current average), the Orchestrator marks its assigned ranges as "stolen." It doesn't kill the slow connectionit simply assigns the same range to a faster worker.

### 3. The Race
- Both connections fetch the same data range in parallel.
- The first connection to deliver the data to the **Storage Engine** wins.
- The **Storage Engine** performs an atomic write and signals completion.
- The Orchestrator immediately sends a `CANCEL` message to the "loser" connection to save bandwidth.

## Adaptive Connection Scaling (ADR 0023)

Aura dynamically scales the number of connections to a source based on performance metrics.

### Global Potential
Aura maintains an internal estimate of your network's capacity, known as the **Global Potential**.
- **Scaling Up**: If the current total throughput is below the Global Potential, the Orchestrator will open more concurrent connections (up to `max_connections_per_task`) to the same source. This is particularly effective for servers that cap speed per-connection (e.g., 100KB/s per stream).
- **Scaling Down**: If increasing connections does not result in higher throughput (e.g., because your own line is saturated), Aura scales back to the `min_connections_per_task` floor to reduce system overhead and avoid being flagged as a bot.
