# ADR 0019: Buffer Pool and Write-Back Caching

## Status
Accepted

## Context
Efficiently handling high-speed downloads (1Gbps+) requires minimizing disk I/O and CPU overhead. The original `aria2` uses a `WrDiskCache` to aggregate small writes into larger, more efficient disk operations.

## Decision
1. **Buffer Pool**: We will implement a centralized `Buffer Pool` using the `bytes` crate. 
2. **Write-Back Strategy**: Data received from **Protocol Workers** will be stored in the Buffer Pool. The **Storage Engine** will only flush this data to disk when:
    - A full **Piece** is received and verified.
    - The total Buffer Pool size exceeds a user-defined limit.
    - A periodic flush timer expires.
3. **Memory Alignment**: Buffers will be aligned to disk sector boundaries where possible to enable Direct I/O optimizations.

## Alternatives Considered
- **Direct Writing**: Writing every packet to disk immediately. *Rejected:* Leads to high disk fragmentation and poor performance on slow disks or at high speeds.
- **System Page Cache**: Relying entirely on the OS page cache. *Rejected:* Doesn't allow us to perform hash verification *before* the data hits the disk, which is a key integrity goal.

## Consequences
- **Pros**: Reduced disk I/O, better throughput on high-latency storage, and centralized memory management.
- **Cons**: Risk of data loss for un-flushed buffers during a crash (mitigated by the `.aura` control file tracking).
