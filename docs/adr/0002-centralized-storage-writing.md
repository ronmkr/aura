# ADR 0002: Centralized Storage Writing and Ownership

## Status
Accepted

## Context
Multiple **Protocol Workers** may attempt to write data to the same **Download Task** (and thus the same underlying files) simultaneously. We need to ensure data integrity, prevent race conditions, and provide a clean interface for disk caching and hashing.

## Decision
The **Storage Engine** will be the sole component with write access to file handles. 
1. **Protocol Workers** fetch data from the network and send it as a message to the **Storage Engine**.
2. The message includes the piece index, offset, and the raw bytes.
3. The **Storage Engine** serializes these requests, applies any caching logic, and performs the physical write.

## Alternatives Considered
- **Direct Worker Writing**: Each worker opens the file and writes to its assigned range. *Rejected:* Requires complex file locking or coordination to prevent overlapping writes (especially for BitTorrent) and makes global disk caching extremely difficult to implement.

## Consequences
- **Pros**: Zero race conditions for file writes, centralized cache management, easy implementation of "hashing-before-writing."
- **Cons**: The **Storage Engine** channel could become a bottleneck if not implemented with sufficient throughput (e.g., using `tokio::mpsc` with appropriate capacity).
