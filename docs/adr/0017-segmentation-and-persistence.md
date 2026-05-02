# ADR 0017: Segmentation and Discovery Persistence

## Status
Accepted

## Context
For protocols that lack a natural piece structure (HTTP, FTP), `aria2` uses a `SegmentMan` to enable parallel downloads. Additionally, background services like DHT require persistent storage of their internal state (routing tables) to avoid slow "cold starts."

## Decision
1. **Segmenter**: We will implement a `Segmenter` that maps a range of bytes to virtual **Pieces**. For HTTP, it will use `Range` headers to fetch specific segments. For FTP, it will use `REST` and `RETR`.
2. **Dynamic Splitting**: The `Segmenter` can dynamically split a segment if a fast worker becomes available (Work Stealing).
3. **Discovery Persistence**: Background actors (DHT, PEX, Tracker caches) will implement a `PersistentState` trait. This state will be serialized to a standard location (e.g., `~/.aura/dht.dat`) using a compact binary format or JSON.

## Alternatives Considered
- **Fixed-size Splitting**: Only split the file at the start. *Rejected:* Doesn't account for servers that don't support Range requests or for mirrors with different speeds.
- **In-memory Discovery**: Re-discover everything on every restart. *Rejected:* Leads to poor user experience, especially for slow-to-connect swarms.

## Consequences
- **Pros**: Parity with `aria2`'s best-in-class multi-source performance and nearly instant resume/discovery after restart.
- **Cons**: Requires managing complex virtual piece boundaries for non-BitTorrent protocols.
