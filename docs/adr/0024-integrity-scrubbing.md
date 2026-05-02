# ADR 0024: Integrity Scrubbing and Torrent Refreshing

## Status
Accepted

## Context
Long-running or "stuck" downloads (especially Torrents) can benefit from a forced re-verification of existing data and a re-announcement to the peer-discovery network.

## Decision
1. **Integrity Scrubber**: We will implement an actor that can be triggered manually or automatically (e.g., when progress stalls). It reads existing data from the **Storage Engine** and re-verifies it against the **Bitfield**.
2. **Peer Discovery Refresh**: Upon a scrub or a manual "refresh" command, the **Orchestrator** will signal all **Discovery Actors** (DHT, Trackers) to perform an immediate, high-priority discovery cycle to find new, potentially faster peers.
3. **Cache Invalidation**: The scrubber will signal the **Buffer Pool** to flush or invalidate cached data for suspected corrupt pieces to ensure a fresh fetch from the network.

## Alternatives Considered
- **Full Task Restart**: Deleting and re-adding the task. *Rejected:* Wastes metadata and connection state; much slower than a targeted scrub.

## Consequences
- **Pros**: Robust recovery from data corruption or "dead" swarms without user-facing complexity.
- **Cons**: Scrubbing is I/O intensive as it requires reading the entire downloaded file from disk.
