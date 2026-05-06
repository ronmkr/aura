# ADR 0009: Global Token Bucket Throttling

## Status
Accepted

## Context
A high-performance download manager must be able to respect user-defined bandwidth limits (global and per-task). In a highly concurrent asynchronous environment, naive sleep-based throttling can lead to thread starvation or imprecise rate control.

## Decision
We will implement a **Hierarchical Token Bucket Throttler** integrated with the **Orchestrated Pull** and **Upload Handler** models.
1. **Mechanism**: The **Orchestrator** maintains a hierarchy of token buckets:
    - **Global Level**: One bucket for total download and one for total upload.
    - **Task Level**: Optional buckets for specific **Download Task** limits.
2. **Admission (Download)**: When a **Protocol Worker** requests work, the **Piece Selector** must acquire tokens from *both* the task-level and global-level buckets.
3. **Admission (Upload)**: When fulfilling a remote peer's request, the worker must acquire tokens from both buckets before transmitting data.
4. **Delay**: If either bucket is empty, the response is delayed until tokens accumulate in both.
5. **Precision**: Tokens are allocated in chunks (e.g., 64KiB) to balance precision and performance.

## Alternatives Considered
- **Worker-level Throttling**: Each worker manages its own speed. *Rejected:* Very difficult to coordinate a global limit across 1,000+ connections accurately.
- **I/O Throttling**: Throttling at the **Storage Engine** level. *Rejected:* Data has already been downloaded by the time it reaches storage; this wastes bandwidth if the limit is exceeded.

## Consequences
- **Pros**: Precise global control, no thread blocking, and zero wasted bandwidth (data isn't fetched if we don't have the "budget").
- **Cons**: Response latency for "work requests" increases when the limit is reached (this is intended behavior).
