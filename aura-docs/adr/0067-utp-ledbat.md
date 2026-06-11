# ADR 0067: μTP/LEDBAT Transport Layer

## Status
Proposed (2026-06-11 — Issue #286)

## Context
Standard TCP-based BitTorrent traffic is aggressive and can saturate a user's network connection, leading to high latency for other applications on the same connection. To address this, the Micro Transport Protocol (μTP, BEP 29) was developed. μTP uses the Low Extra Delay Background Transport (LEDBAT) congestion control algorithm over UDP. LEDBAT measures queueing delay and dynamically backs off when congestion is detected, making it friendly to other interactive traffic. Additionally, many high-speed seeders and swarms on private/public trackers operate exclusively on μTP (UDP) connections, which means Aura is currently unable to connect to them.

## Decision
1. Implement μTP support in the transport layer of `aura-core/src/transport/`.
2. Evaluate existing Rust μTP crate implementations (such as `utp` or similar) to avoid writing from scratch, wrapping them under Aura's native transport abstraction.
3. Integrate the LEDBAT congestion control algorithm, which dynamically adjusts congestion window size based on measured loopback delay vs. a target delay (typically 100ms).
4. Establish configurable preference in `Aura.toml`: `[bittorrent] prefer_utp = true` (default `true`).
5. Plumb μTP into the peer connection manager. When connecting to a peer, attempt a μTP handshake over UDP first. If the UDP handshake fails or timeouts, fall back to TCP.
6. Share a single UDP socket across all active BitTorrent tasks to multiplex inbound/outbound μTP packets using the peer's IP/port and connection ID.

## Edge Cases
1. **Firewalls & NATs**: UDP packets are frequently dropped or blocked on restrictive firewalls. Robust fallback to TCP is mandatory to ensure reliability.
2. **Packet Reordering & Loss**: Because UDP is connectionless, μTP must handle packet loss, reordering, and duplicate packets internally using sequence numbers and ACKs.
3. **UDP Buffer Saturation**: High-speed UDP packet sending can overwhelm the OS UDP receive buffer. Socket buffers must be sized appropriately on startup.

## Alternatives Considered
- **TCP Only**: Continue with standard TCP connections only. *Rejected:* Leaves Aura unable to connect to UDP-only peers and susceptible to network performance degradation under full link utilization.
- **Strict Throttling**: Rely solely on static token bucket limits (ADR-0009) to avoid congestion. *Rejected:* Static throttling is not dynamic; it does not adjust to changing network conditions or other traffic on the LAN.

## Consequences
- **Pros**: Dynamic back-off prevents network saturation; enables communication with UDP-only peers; makes Aura a good network citizen.
- **Cons**: Substantial implementation complexity in UDP multiplexing and packet state management.
