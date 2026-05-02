# ADR 0031: BitTorrent v2 and Merkle Tree Management

## Status
Accepted

## Context
BitTorrent v2 (BEP 52) replaces SHA-1 with SHA-256 and introduces per-file Merkle trees for data verification. For large torrents (terabytes of data), these trees can become significantly large and cannot be kept entirely in memory.

## Decision
1. **Merkle Tree Store**: We will implement a specialized storage layer for hash trees using an embedded key-value store (e.g., `sled` or `rocksdb`) or a memory-mapped file structure.
2. **Lazy Loading**: Hash nodes will be loaded into RAM only when needed for piece verification and will be cached using an LRU (Least Recently Used) policy.
3. **Upgrade Path**: The **Storage Engine** will support "Hybrid Torrents" (v1 + v2), maintaining both SHA-1 and SHA-256 bitfields during the transition period.

## Alternatives Considered
- **In-Memory Trees**: Storing all hashes in a `Vec<u8>`. *Rejected:* Scales poorly for massive datasets.
- **Flat File Storage**: Storing trees in plain files. *Rejected:* Inefficient for random access and updates.

## Consequences
- **Pros**: Support for modern P2P standards, efficient verification of massive files, and reduced memory footprint.
- **Cons**: Increased implementation complexity for the storage engine and added dependency on a KV store.
