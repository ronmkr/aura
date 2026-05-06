# ADR 0026: Modern Networking (Happy Eyeballs, Alt-Svc, Streaming)

## Status
Accepted

## Context
Modern networking requires minimizing latency through parallel connection attempts and utilizing the fastest available protocols (HTTP/3). Additionally, users want to consume media while it is still downloading.

## Decision
1. **Happy Eyeballs (RFC 8305)**: The **Proxy Connector** will implement Happy Eyeballs, initiating parallel connection attempts for IPv4 and IPv6 and selecting the fastest responder.
2. **Alt-Svc Resolution**: **Protocol Workers** will parse `Alt-Svc` headers. If an HTTP/1.1 or HTTP/2 response indicates the availability of an HTTP/3 (QUIC) endpoint, the **Orchestrator** will attempt to migrate the task to a QUIC-enabled worker.
3. **Prioritized Streaming**: The **Piece Selector** will support a `Streaming` mode that prioritizes pieces at the beginning and end of the file, facilitating media header parsing and linear playback.

## Alternatives Considered
- **Serial Connection Attempts**: Trying IPv6, waiting for timeout, then trying IPv4. *Rejected:* Adds significant latency on misconfigured dual-stack networks.
- **Fixed Protocol Selection**: Only using the protocol specified in the URI. *Rejected:* Misses performance opportunities provided by modern server upgrades (HTTP/3).

## Consequences
- **Pros**: Lower connection latency, automatic protocol upgrades, and enhanced media consumption experience.
- **Cons**: Implementing RFC 8305 and HTTP/3 (QUIC) adds significant complexity to the network stack.
