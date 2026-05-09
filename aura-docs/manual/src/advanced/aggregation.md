# Multi-Protocol Aggregation

Aura's most powerful feature is its ability to treat multiple disparate sources as a single logical download. This process is managed by the **Sequential Aggregator**.

## The Sourced Model

A `MetaTask` in Aura consists of one or more subtasks. Each subtask represents a specific source (e.g., an HTTP mirror, an FTP server, or a BitTorrent swarm).

### Supported Combinations
- **HTTP + HTTP**: Speed up downloads by hitting multiple mirrors.
- **HTTP + BitTorrent**: Use stable mirrors to "seed" a swarm or fill in missing pieces.
- **FTP + BitTorrent**: Aggregate high-speed FTP sources with P2P swarms.

## Racing Work Stealer

To prevent a single slow mirror from bottlenecking the entire download, Aura implements a **Racing Work Stealer** (ADR 0005).

- **Throughput Monitoring**: The Orchestrator calculates the Exponential Weighted Moving Average (EWMA) throughput for every connection.
- **Speculative Stealing**: If a connection is significantly slower (3x slower than the average), the Orchestrator marks its pending ranges as "stolen."
- **Racing**: A faster connection is assigned the stolen range. The first connection to finish the range wins, and the "loser" is immediately canceled to prevent redundant disk writes.

## Adaptive Connection Scaling

Aura doesn't just open a fixed number of connections. It dynamically scales based on performance (ADR 0023).

- **Scaling Up**: If the total throughput is below the "Global Potential" (the sum of known source capacities), Aura will open more concurrent connections to the same source (bypassing per-connection caps).
- **Scaling Down**: If increasing connections does not increase throughput, Aura scales back to save system resources and prevent being flagged as a bot by mirrors.
