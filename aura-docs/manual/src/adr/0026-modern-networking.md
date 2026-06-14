# Decision 0026: Modern Networking (Happy Eyeballs, Alt-Svc, Streaming)

## Status

Implemented (2026-06-10, PR #277)

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

## Implementation Details

1. **HTTP/3 Support**: Enabled via the `reqwest` crate's `"http3"` feature and workspace-level global `--cfg reqwest_unstable` compilation configuration.
2. **Alt-Svc Caching**: Built a persistent [alt_svc.rs](../../aura-core/src/security/alt_svc.rs) cache that serializes to `.aura/alt_svc.json`. This stores alternative protocols, hosts, ports, and max-age expiration.
3. **Transparent Fallback**: To handle hostile environments where UDP/443 is blocked or filtered, the worker automatically falls back to standard HTTP/1.1 or HTTP/2 over TCP if the HTTP/3 handshake or request fails.

## Implementation Status (audit 2026-06-03)

- **Happy Eyeballs (RFC 8305)**: Implemented in DNS racing (2026-05-29, PR #140).
- **Alt-Svc Resolution & HTTP/3**: Implemented in HTTP/3 QUIC and Alt-Svc support (2026-06-03, PR #198).
- **Connection Pool Sharing**: Shared reqwest HTTP connection pool across segment workers implemented in Issue #256.
- **Prioritized Streaming**: Implemented (Issue #28, PR #277).
