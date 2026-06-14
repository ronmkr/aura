# Decision 0015: URL Globbing and Batch Processing

## Status

Implemented (2026-05-06, commit 0777b1ab)

## Context

Users often need to download a sequential series of files (e.g., `image001.jpg` to `image100.jpg`). Manually adding 100 URIs is inefficient. `curl` provides a powerful globbing syntax for this.

## Decision

1. **URL Globber**: We will implement a `URLGlobber` component in the **Orchestrator** (using a crate like `glob` or custom regex expansion).
2. **Expansion**: When a URI containing brackets `[]` or braces `{}` is received, the Globber expands it into a list of URIs.
3. **Batch Tasks**: Each expanded URI is treated as a separate **Download Task**.
4. **Shared Options**: All tasks in a batch share the same initial configuration but can be controlled individually once created.
5. **Crawler Integration**: Seed URLs passed to the recursive crawler (defined under [Decision-0030](0030-recursive-mirroring.md)) are also expanded using the glob expansion logic during crawler initialization.

## Alternatives Considered

- **Worker-level Globbing**: Have the worker handle the glob. *Rejected:* Violates the principle that a worker handles exactly one URI at a time and would break progress tracking.

## Consequences

- **Pros**: Parity with `curl`'s most powerful CLI features and significantly improved UX for batch downloads.
- **Cons**: Can lead to a sudden spike in **Download Tasks** if a large range is specified (e.g., `[0-9999]`). We may need a safety limit.
