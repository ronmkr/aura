# Decision 0052: Allocation Prober Diagnostic Tool

## Status

Implemented (2026-06-02, PR #196)

## Context

Standard file pre-allocation methods like `fallocate` can have inconsistent performance or cause regressions on certain filesystems (especially network mounts or exotic local filesystems). Currently, Aura uses a hardcoded strategy which might not be optimal for the user's specific storage environment.

## Decision

Implement an `AllocationProber` diagnostic tool that:
1. Measures real-world disk write speed for various allocation methods (`fallocate`, sparse files with holes, full zero-fills).
2. Runs automatically (or via command) to determine the best method for the current mount point.
3. Integrates with the `StorageEngine` to select the most performant strategy dynamically.

## Consequences

- **Pros**: Optimal performance across any filesystem.
- **Cons**: Adds complexity to the storage layer and potential small delay during initial probing.
