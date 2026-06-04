Status: Implemented

# ADR 0016: RPC Server and Interface Binding

## Status
Implemented (2026-05-06, commit 0777b1ab)

## Context
`Aura` must be controllable by remote UIs and provide fine-grained control over network routing (parity with `curl`).

## Decision
1. **RPC Server**: We will implement a dedicated actor using `warp` or `axum` that supports JSON-RPC 2.0 and WebSockets. It will be disabled by default for security.
2. **Bi-directional Telemetry**: The RPC Server will subscribe to the **Event Bus** and push events to WebSocket clients in real-time.
3. **Interface Binder**: We will use the `socket2` crate to bind sockets to specific network interfaces or local IP addresses before establishing connections. This logic will live in the **Proxy Connector**.
4. **Dual-Stack Support**: The system will default to IPv6 but allow users to force IPv4 or specify a preference via the **Configuration Manager**.

## Implementation Status (Audit 2026-06-03)
- **RPC Server & Interface Binding**: Initially implemented in commit `0777b1ab` (2026-05-06).
- **WebSocket Telemetry**: Fully implemented via PR #106 (2026-05-28).
- **Security Gaps**: Audit identified that the RPC daemon binds to `0.0.0.0` by default (GAP-07b / Issue #202), uses a public hardcoded default token (GAP-07a / Issue #201), has permissive CORS (GAP-41 / Issue #203), and does not support TLS (GAP-47). A security hardening effort is required to address these (see ADR 0056).

## Alternatives Considered
- **Direct Orchestrator RPC**: Making the Orchestrator also an HTTP server. *Rejected:* Violates single-responsibility principle and makes the main loop too heavy.

## Consequences
- **Pros**: Full compatibility with standard frontends and WebUIs, and advanced networking support for VPN/Multi-homed users.
- **Cons**: Increases the attack surface of the application; requires robust authentication (Token/Secret) for the RPC layer.
