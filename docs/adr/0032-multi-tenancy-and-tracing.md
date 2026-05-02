# ADR 0032: Multi-Tenancy and Observability

## Status
Accepted

## Context
As a headless daemon, `Aura` may be shared by multiple users or applications. We need a way to isolate their tasks and provide deep visibility into the performance of the engine's many concurrent actors.

## Decision
1. **Tenant Context**: The **Orchestrator** will support a `TenantContext` that wraps sets of **Download Tasks**. Each context can have its own speed limits, disk path root, and authentication token.
2. **Observability Spans**: We will use the `tracing` crate to instrument every actor. Every **Piece Request** will be tagged with a unique `request_id`, allowing us to trace it through the Selector, Worker, and Storage Engine.
3. **OpenTelemetry Integration**: The engine will optionally export these spans to standard formats (e.g., Jaeger, Honeycomb) for performance debugging.

## Alternatives Considered
- **Namespace-only isolation**: Only separating tasks by name. *Rejected:* Doesn't provide the resource control required for multi-user systems.
- **Traditional Logging**: *Rejected:* Insufficient for debugging race conditions or performance bottlenecks in an async actor system.

## Consequences
- **Pros**: Safe multi-user operation, world-class debuggability, and easier performance tuning.
- **Cons**: Instrumentation adds a small amount of runtime overhead and requires careful management of trace data volume.
