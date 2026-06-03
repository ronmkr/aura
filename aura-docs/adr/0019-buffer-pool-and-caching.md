# ADR 0019: Buffer Pool and Write-Back Caching

## Status
Superceded (by Issue #160, 2026-05-30, PR #164)

## Context
Efficiently handling high-speed downloads (1Gbps+) requires minimizing disk I/O and CPU overhead. Older architectures use a `WrDiskCache` to aggregate small writes into larger, more efficient disk operations.

## Decision (Original)
1. **Buffer Pool**: We will implement a centralized `Buffer Pool` using the `bytes` crate. 
2. **Write-Back Strategy**: Data received from **Protocol Workers** will be stored in the Buffer Pool. The **Storage Engine** will only flush this data to disk when:
    - A full **Piece** is received and verified.
    - The total Buffer Pool size exceeds a user-defined limit.
    - A periodic flush timer expires.

## Resolution (2026-05-27)
Following an architectural audit (Issue #160), the dedicated `BufferPool` actor was **removed**. 

### Rationale:
- **Redundancy**: The `bytes` crate (`BytesMut`) already provides atomic reference counting and efficient memory management. Wrapping it in a shallow pool added significant cross-module coupling without measurable performance gains.
- **Locality**: Workers now allocate `BytesMut` directly. The `StorageEngine` processes these and allows them to fall out of scope for automatic cleanup.
- **Zero-Copy**: Zero-copy is still achieved by passing the `BytesMut` buffer from the worker to the storage engine via async channels.

## Alternatives Considered
- **Centralized Pool**: (Rejected) High complexity, low reward for the current scale.
- **System Page Cache**: Relying entirely on the OS page cache. (Rejected) Still need write-back aggregation for sequential I/O.

## Consequences
- **Pros**: Simplified codebase, reduced coupling, same zero-copy performance.
- **Cons**: Global memory limits must now be enforced by the `ResourceGovernor` tracking outstanding `BytesMut` allocations rather than a single pool.
