# ADR 0033: Generation-based Writes and Sequential Aggregation

## Status
Accepted

## Context
High-concurrency downloads with **Racing** (fetching the same piece twice) and **Work Stealing** create the risk of "Zombie Writes," where a slow or late worker overwrites a faster one's data. Additionally, non-sequential writes (common in BitTorrent) degrade physical disk performance.

## Decision
1. **Generation Tracking**: Every time a **Piece** is assigned to a **Protocol Worker**, the **Orchestrator** increments its **Generation ID**.
2. **Atomic Commit**: The **Storage Engine** will only accept a **Write Request** if its Generation ID matches the current active generation for that piece index. Late arrivals from the "loser" of a race are discarded immediately at the actor mailbox level.
3. **Sequential Aggregator**: The **Storage Engine** will implement a sliding window of pending writes. It will prefer flushing contiguous blocks of pieces to disk (e.g., pieces 1, 2, 3 together) rather than random pieces, even if they arrive out of order.
4. **Memory Pressure**: If the **Buffer Pool** exceeds its limit, the aggregator will perform a "Forced Flush" of the oldest pending pieces to disk, regardless of continuity.

## Alternatives Considered
- **File Locking**: Locking byte ranges in the file. *Rejected:* Too much overhead and difficult to manage across different OS implementations.
- **Immediate Disk Write**: Writing exactly when packets arrive. *Rejected:* Leads to physical fragmentation and poor performance on NAS/HDD.

## Consequences
- **Pros**: Zero race conditions for speculative writes, significantly higher I/O throughput on non-SSD storage, and reduced filesystem fragmentation.
- **Cons**: Increases the complexity of the **Storage Engine** state machine and uses more RAM to buffer the "Sequential Window."
