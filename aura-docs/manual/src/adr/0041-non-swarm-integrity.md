# Decision 0041: Integrity Verification for Non-Swarm Protocols

## Status
Implemented (2026-05-27, PR #112)

## Context
While BitTorrent has built-in hash verification, HTTP and FTP protocols rely on transport-layer reliability, which does not protect against data corruption at rest or mirror-side errors. Modern download managers should provide an optional post-download verification step.

## Decision
1. **Checksum Strategy**: Aura will support MD5, SHA-1, SHA-256, and SHA-512 verification for any task.
2. **Verification Phase**: A new phase, `Verifying`, will be added to the **Task Lifecycle**. This phase occurs after 100% data retrieval but before moving to `Completed`.
3. **Streaming Hashing**: To optimize I/O, the **Storage Engine** will perform streaming hashing during the final data flush if a checksum is provided upfront. If provided after completion, a dedicated **Integrity Scrubber** (see Decision 0024) will be used.
4. **Mismatch Policy**: If a checksum mismatch is detected, the task will move to `Failed` and keep the `.part` file, allowing users to manually inspect or force a re-download.

## Alternatives Considered
- **External Verification**: Letting the user run `sha256sum`. *Rejected:* Poor UX; Aura should be a self-contained, reliable tool.
- **In-flight Hashing only**: *Rejected:* Cannot verify if the checksum is provided after the download starts.

## Consequences
- **Pros**: Guaranteed data integrity for all protocols, parity with standard file checksum utilities.
- **Cons**: Adds CPU overhead for hashing and potentially an extra full-file read pass if hashing wasn't done during the download.

## Implementation
- **Non-Swarm Integrity**: Implemented in `aura-core/src/storage/ops.rs` and `aura-core/src/orchestrator/lifecycle/` (2026-05-27, PR #112).
