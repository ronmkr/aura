# ADR 0001: Orchestrated Pull Model for Work Assignment

## Status
Accepted

## Context
In `Aura`, the system must distribute piece requests across multiple protocols (HTTP, BitTorrent) and workers efficiently. We need a communication pattern that supports backpressure, centralized strategy (like "Rarest First"), and "Work Stealing" (re-assigning slow pieces).

## Decision
We will use an **Orchestrated Pull** model. 
1. **Protocol Workers** are responsible for managing their own connection state and capacity.
2. When a worker has capacity, it sends a message to the **Orchestrator** requesting work.
3. The **Piece Selector** (within the Orchestrator) selects the best piece based on the global state and responds to the worker.

## Alternatives Considered
- **Direct Push**: The Orchestrator pushes work to workers. *Rejected:* Risk of overwhelming workers and complex buffer management.
- **Decentralized Pull**: Workers pick their own pieces from a shared bitfield. *Rejected:* Logic fragmentation; difficult to implement global optimizations like Work Stealing across different protocols.
- **Reactive Stream**: A constant stream of work with a worker-managed budget. *Rejected:* Higher implementation complexity for initial phase.

## Consequences
- **Pros**: Clear backpressure, centralized scheduling logic, easier implementation of Work Stealing.
- **Cons**: Adds one round-trip of message latency (request for work) before fetching starts.
