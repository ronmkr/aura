# ADR 0006: Error Classification and Self-healing Strategy

## Status
Accepted

## Context
Downloads often fail due to transient network issues or server errors. A robust download manager must distinguish between errors that can be automated (retries, protocol switching) and those that require user or system-level intervention.

## Decision
1. **Error Classification**: We will implement a three-tier error hierarchy (Worker, Task, Engine) as defined in `CONTEXT.md`.
2. **Policy Manager**: A dedicated component within the **Orchestrator** will evaluate errors against a set of policies.
3. **Self-healing**: 
    - **Worker Errors** are handled by returning the piece to the pool and cooling down the specific URI/Peer.
    - **Task Errors** trigger automatic failover strategies, such as switching to an alternative protocol (e.g., if an HTTP Metalink provides a BitTorrent Magnet fallback).
4. **Resumption State**: All state required for self-healing (retry counts, failed URIs) must be persisted in the control file to survive restarts.

## Alternatives Considered
- **Uniform Error Handling**: Treat all errors as task-level failures. *Rejected:* Leads to poor user experience (frequent manual resumes for minor blips).
- **Hardcoded Failovers**: Embedding retry logic inside Protocol Workers. *Rejected:* Makes it difficult to implement global policies or cross-protocol logic.

## Consequences
- **Pros**: High resilience, reduced manual intervention, and clear separation of concerns between protocol logic and recovery logic.
- **Cons**: Increased complexity in the **Orchestrator** to track and manage the retry/failover state machines.
