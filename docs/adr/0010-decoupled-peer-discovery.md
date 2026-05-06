# ADR 0010: Decoupled Peer Discovery and Registry

## Status
Accepted

## Context
BitTorrent peer discovery is a multi-faceted process involving Trackers (HTTP/UDP), DHT (Kademlia), and PEX (Protocol-level exchange). In `aria2`, this logic is tightly integrated. For `Aura`, we need a modular approach that allows these mechanisms to run independently while feeding a central "candidate pool" for each download.

## Decision
We will decouple peer discovery from peer management:
1. **Discovery Actors**: Background actors (DHT, Tracker Manager) run globally or per-task. They discover IP/Port pairs and send them to the **Orchestrator**.
2. **Peer Registry**: Each BitTorrent **Download Task** maintains its own **Peer Registry**. This registry is the "Source of Truth" for peer health and availability.
3. **Reputation**: The Registry tracks peer performance (throughput, data integrity). If a **Protocol Worker** reports a hash failure, the Registry lowers that peer's **Reputation**.
4. **Assignment**: **Protocol Workers** request new peer candidates from the **Peer Registry** when they have an available connection slot.

## Alternatives Considered
- **Worker-managed Discovery**: Each worker performs its own PEX/Tracker requests. *Rejected:* Leads to redundant network traffic and poor global coordination.
- **Global Peer Pool**: A single pool for all downloads. *Rejected:* Peer discovery is usually specific to an info-hash; a global pool would require complex filtering.

## Consequences
- **Pros**: Clear separation of discovery logic from protocol logic, easier implementation of "banning" malicious peers, and better control over the total number of connections.
- **Cons**: Requires message-passing between Discovery Actors and the Task's Peer Registry, adding a small amount of internal overhead.
