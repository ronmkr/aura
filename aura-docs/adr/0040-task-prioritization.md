Status: Implemented

# ADR 0040: Task Prioritization and Dependency Chains

## Status
Implemented (2026-06-03, PR #199)

## Context
As the number of active tasks increases, Aura needs a way to intelligently allocate system resources (bandwidth, disk I/O, connection slots). Users often have high-priority downloads that should finish before others, or sequences of downloads where one depends on the success of another.

## Decision
1. **Priority Levels**: We will implement a 6-level priority system (0-5, where 0 is highest). The **Throttler** will use these weights to proportionally allocate bandwidth.
2. **Dependency Graph**: The **Orchestrator** will maintain a directed acyclic graph (DAG) of task dependencies. A task in the "Waiting" state will only transition to "Initializing" once all its parent tasks have moved to "Completed".
3. **Resource Preemption**: High-priority tasks (Priority 0) can preempt lower-priority tasks, forcing them to pause or yield connection slots if system-wide limits are reached.
4. **RPC Extension**: The JSON-RPC API will be extended to allow setting `priority` and `depends_on` (list of GIDs) at task creation or during runtime.

## Alternatives Considered
- **Strict Sequential Order**: Simplest to implement but lacks the flexibility of weights and parallel high-priority streams.
- **Dynamic Scoring**: Automatically adjusting priority based on swarm health. *Rejected:* Too complex for initial implementation; manual priority is more predictable for users.

## Consequences
- **Pros**: Better resource utilization, user control over critical downloads, and support for complex batch sequences.
- **Cons**: Managing the DAG adds complexity to the Orchestrator's state machine and requires cycle detection.
