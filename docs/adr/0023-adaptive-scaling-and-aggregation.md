# ADR 0023: Adaptive Connection Scaling and Sourced Aggregation

## Status
Accepted

## Context
Many file-hosting servers (Rapidshare, Megashare) and even standard HTTP servers apply per-connection speed limits to free users or as a general throttling measure. Additionally, users often have access to a file via multiple protocols (e.g., a Torrent and an HTTP mirror).

## Decision
1. **Adaptive Connection Scaling**: The **Orchestrator** will monitor the speed of individual connections. If a connection is consistently slow but the global bandwidth limit has not been reached, the **Segmenter** will split the task into more segments, effectively creating more parallel connections to bypass the per-connection cap.
2. **Sourced Aggregation**: A single **Download Task** will be able to manage multiple protocol workers of different types (HTTP, BT, NNTP) simultaneously. The **Piece Selector** will distribute piece requests across all sources, prioritizing those with higher throughput.
3. **Usenet Integration**: We will implement an **NNTP Worker** that treats an NZB file as a metadata source (similar to a Torrent or Metalink), allowing Usenet to be one of the sources in an aggregated download.

## Alternatives Considered
- **Static Connection Count**: Fixed number of connections (e.g., `-x 16`). *Rejected:* Doesn't adapt to server behavior; can lead to being banned for "excessive connections" if used blindly.
- **Protocol Isolation**: Only one protocol per task. *Rejected:* Prevents using high-speed Usenet or HTTP mirrors to speed up slow BitTorrent swarms.

## Consequences
- **Pros**: Dramatically faster downloads by bypassing artificial limits and combining the bandwidth of multiple sources.
- **Cons**: High connection counts can increase CPU usage and risk temporary IP bans from strict servers.
