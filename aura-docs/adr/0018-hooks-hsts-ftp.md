Status: Implemented

# ADR 0018: Hooks, HSTS, and Multi-Channel Protocols

## Status
Implemented (2026-05-06, commit 0777b1ab)

## Context
`Aura` must support user automation (hooks), modern security (HSTS), and complex legacy protocols (FTP Active Mode).

## Decision
1. **Hook Manager**: The **Orchestrator** will use a `HookManager` to execute external commands. Hooks are defined in the **Configuration Manager** and triggered by **Telemetry Events**.
2. **Security Policy**: The **Security Context** will maintain an `HSTSCache` (persisted to disk). If a domain is marked as HSTS-only, all future HTTP requests to that domain will be automatically upgraded to HTTPS.
3. **Multi-Channel Workers**: For protocols like FTP, the **Protocol Worker** will manage multiple async tasks (one for control, one for each data channel). This encapsulates the complexity of port negotiation away from the core engine.

## Implementation Status (Audit 2026-06-03)
- **Hook Manager**: Fixed and fully integrated via PR #109 (2026-05-18).
- **HSTS Cache**: Fully implemented via PR #131 (2026-05-28).
- **FTP Support**: Initially implemented in commit `0777b1ab` (2026-05-06) and extended via PR #133 (2026-05-28).

## Alternatives Considered
- **Direct Script Execution**: Hardcoding script calls in the task logic. *Rejected:* Difficult to test and maintain.
- **Protocol-specific TLS**: Letting each worker manage its own security. *Rejected:* Leads to inconsistent security policies and redundant certificate management.

## Consequences
- **Pros**: Stronger security posture, rich user automation, and clean support for complex network protocols.
- **Cons**: Managing HSTS state adds persistent state overhead; multi-channel FTP is notoriously difficult to implement in async/NAT-heavy environments.
