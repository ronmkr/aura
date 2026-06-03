# ADR 0057: ResourceGovernor for Global Memory Backpressure

## Status
Implemented (2026-06-03, Issue #207)

## Context
Following the removal of the dedicated `BufferPool` actor (ADR 0019), memory allocation in Aura is decentralized. Protocol Workers allocate `BytesMut` buffers directly and pass them down channels to the Storage Engine. Without a central tracking system, extreme high-speed downloads or large swarm populations can lead to unbounded allocations in channel buffers and memory queues, risking Out-Of-Memory (OOM) process crashes (GAP-01).

## Decision
1. **ResourceGovernor Component**: Implement a thread-safe `ResourceGovernor` shared across the application via `Arc`.
2. **Atomic Allocation Tracking**: The governor tracks outstanding allocated bytes using atomic counters.
3. **Allocation Hook/Gate**: Protocol Workers must request a memory budget allocation before allocating large piece buffers.
4. **Piece Picking Backpressure**: Integrate the resource governor with the `PiecePicker` in the `Orchestrator`. If the memory budget is exceeded, the `PiecePicker` will refuse to assign new pieces to requesting workers, forcing workers to pause requests until the storage queue drains and memory is freed.
5. **Configurable Memory Limits**: Enforce configurable hard limits and soft limits (e.g., 512MB default limit) via `Aura.toml`.

## Edge Cases
1. **Deadlock from Complete Memory Saturation**: If the memory limit is strictly enforced and channels are blocked, workers waiting to flush metadata might be blocked by the `PiecePicker` refusing allocations, creating a deadlock. To prevent this, metadata allocations and block validation tasks will have a reserved "safety margin" that cannot be choked by standard piece downloads.
2. **Extremely Large Pieces**: In BitTorrent v2, piece sizes can scale up to 16MB or 32MB. Assigning a single piece to a worker on a low-memory device could immediately breach limits. The governor must track pending bytes per active worker and adjust the window size (max pipeline requests) dynamically.
3. **Multi-Tenant Starvation**: A single high-speed torrent task from tenant A could consume the entire global memory budget, starving tenant B's tasks. The ResourceGovernor will track memory allocations per tenant/task and enforce a fair-share allocation algorithm.

## Alternatives Considered
- **Reintroduce BufferPool Actor**: Re-wrapping all writes in a single actor. *Rejected:* Re-introduces architectural coupling and complexity. The governor is simpler and acts as a passive supervisor.
- **Fixed Channel Limits**: Relying purely on bounding tokio channels. *Rejected:* Hard to coordinate a single channel limit across many dynamic peers/tenants, and doesn't account for piece size variability.

## Consequences
- **Pros**: Strong protection against OOM crashes; elegant backpressure propagation from disk writing up to network fetching.
- **Cons**: Minor overhead for atomic checking on piece allocation paths.
