# ADR 0004: Telemetry and Event Bus Architecture

## Status
Accepted

## Context
Multiple external interfaces (Ratatui TUI, JSON-RPC, logging) need real-time updates on download progress and system state. Directly polling the **Orchestrator** or individual **Download Tasks** would create contention and complexity.

## Decision
1. **Source of Truth**: The **Orchestrator** is the sole source of truth for the system's state. It aggregates updates from **Protocol Workers** and the **Storage Engine**.
2. **Event Bus**: We will use a `tokio::sync::broadcast` channel to implement an **Event Bus**.
3. **Telemetry Events**: The Orchestrator will convert internal state changes into structured **Telemetry Events** and publish them to the Event Bus.
4. **Subscription**: Subsystems like the TUI and RPC server will subscribe to this broadcast channel to react to changes.

## Alternatives Considered
- **Polling**: TUI/RPC periodically query the Orchestrator. *Rejected:* Inefficient and leads to "stuttering" in the UI.
- **Direct Callbacks**: Subsystems register callbacks with the Orchestrator. *Rejected:* Harder to manage in an async/actor environment; leads to tight coupling.

## Consequences
- **Pros**: Decouples the core engine from its interfaces, allows for multiple simultaneous observers, and ensures a consistent view of the system state.
- **Cons**: Broadcast channels can "lag" if a subscriber is too slow (though Tokio handles this by dropping old messages for that specific subscriber).
