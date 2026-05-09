# ADR 0007: Protocol Encapsulation and Black-Box Workers

## Status
Accepted

## Context
As a multi-protocol download manager, `Aura` must handle diverse networking mechanics (e.g., BitTorrent peer state, HTTP/3 QUIC streams, FTP control channels). Spreading this logic into the **Orchestrator** would lead to a "god object" and make it difficult to add new protocols.

## Decision
**Protocol Workers** will operate as encapsulated "black boxes" relative to the **Orchestrator**.
1. **Interface**: The Orchestrator interacts with workers through a uniform message interface (e.g., `RequestPiece`, `CancelPiece`, `UpdateMetadata`).
2. **State Privacy**: Protocol-specific states (e.g., BitTorrent "Choked" status, HTTP cookies, FTP login state) are entirely private to the worker.
3. **Abstraction**: The **Orchestrator** and **Piece Selector** only care about whether a worker is "available" to fetch data, not the internal reasons for its availability or unavailability.

## Alternatives Considered
- **Shared State**: Workers and Orchestrator share a common state object for peer/connection info. *Rejected:* Leads to complex locking and high coupling.
- **Protocol-Aware Orchestrator**: The Orchestrator has branches for `if protocol == BitTorrent { ... }`. *Rejected:* Violates Open/Closed principle and scales poorly.

## Consequences
- **Pros**: Clean separation of concerns, easier testing (workers can be mocked), and simplified implementation of new protocols (Cloud/S3/IPFS).
- **Cons**: The Orchestrator has less visibility into *why* a worker might be performing poorly, relying instead on high-level metrics like throughput.
