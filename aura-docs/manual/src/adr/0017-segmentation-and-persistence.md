# Decision 0017: Segmentation and Discovery Persistence

## Status

Implemented (2026-06-04, PR #259 — Issues #210, #255, #256)

## Context

For protocols that lack a natural piece structure (HTTP, FTP), standard engines use a segment manager to enable parallel downloads. Additionally, background services like DHT require persistent storage of their internal state (routing tables) to avoid slow "cold starts."

## Decision

1. **Segmenter**: We will implement a `Segmenter` that maps a range of bytes to virtual **Pieces**. For HTTP, it will use `Range` headers to fetch specific segments. For FTP, it will use `REST` and `RETR`.
2. **Dynamic Splitting**: The `Segmenter` can dynamically split a segment if a fast worker becomes available (Work Stealing).
3. **Discovery Persistence**: Background actors (DHT, PEX, Tracker caches) will implement a `PersistentState` trait. This state will be serialized to a standard location (e.g., `~/.aura/dht.dat`) using a compact binary format or JSON, and the DHT actor will periodically (every 5-10 minutes) re-ping known high-uptime nodes to refresh its routing table Kademlia-style.

## Implementation Status

- **Segmenter**: HTTP range fetching is implemented inline in `worker/http/segment.rs`, with connection sharing across workers added in PR #259 (Issue #256).
- **Discovery Persistence**: The `PersistentState` trait and DHT routing table serialization to `dht.dat` are fully implemented (PR #220 / Issue #210).
- **TaskState Persistence**: ETag and Last-Modified persistence for conditional GETs implemented in PR #259 (Issue #255).

## Alternatives Considered

- **Fixed-size Splitting**: Only split the file at the start. *Rejected:* Doesn't account for servers that don't support Range requests or for mirrors with different speeds.
- **In-memory Discovery**: Re-discover everything on every restart. *Rejected:* Leads to poor user experience, especially for slow-to-connect swarms.

## Consequences

- **Pros**: Parity with best-in-class multi-source performance and nearly instant resume/discovery after restart.
- **Cons**: Requires managing complex virtual piece boundaries for non-BitTorrent protocols.
