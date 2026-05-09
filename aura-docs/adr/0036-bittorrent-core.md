# ADR 0036: BitTorrent Core and Swarm Management

## Status
Accepted

## Context
Adding BitTorrent support requires managing complex piece-level progress (Bitfields) and highly concurrent peer discovery and communication. We need to integrate this into our existing actor model without creating a monolithic "Torrent" object.

## Decision
1. **Bitfield**: We will implement a `Bitfield` struct using the `bitvec` crate or a custom bit-packed `Vec<u8>`. It will be owned by the **Download Task**.
2. **Piece Selector (Rarest-First)**: The Orchestrator will implement a "Rarest-First" selection strategy. It will maintain a global view of piece availability across all reported peer bitfields.
3. **Peer Registry**: Each BitTorrent task will have a `PeerRegistry` actor. Discovery actors (DHT, Trackers) send `NewPeer` messages to the registry. The registry manages the lifecycle of `ProtocolWorker` instances for that task.
4. **Sub-Piece Requests**: To optimize throughput, workers will request small "blocks" (typically 16KB) within a "Piece" (typically 1MB+). The **Storage Engine** will aggregate these blocks into the **Buffer Pool** before performing a full piece hash check.

## Alternatives Considered
- **Direct Worker Discovery**: Letting each worker find its own peers. *Rejected:* Leads to inefficient swarm management and redundant discovery traffic.

## Consequences
- **Pros**: Modular P2P stack, efficient swarm coordination, and clear separation between "Discovery" and "Retrieval."
- **Cons**: Requires complex synchronization between the Piece Selector and the multiple Peer Protocol Workers.
