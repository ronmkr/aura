# ADR 0003: Atomic Completion and Pre-allocation Strategy

## Status
Accepted

## Context
Users should not see partially downloaded or corrupt files in their destination folders. Additionally, long-running downloads should not fail due to disk exhaustion after significant time/bandwidth has already been invested.

## Decision
1. **Atomic Completion**: The **Storage Engine** will append a `.part` suffix (e.g., `.aura-part`) to all active downloads. Only upon 100% verification will the file be renamed to its final target name.
2. **Control Files**: Progress and metadata will be stored in a companion `.aria2` or `.json` file to allow resumption.
3. **Pre-allocation**: By default, the system will attempt to pre-allocate the full file size using platform-specific calls (e.g., `fallocate` on Linux) to guarantee space and reduce fragmentation.

## Alternatives Considered
- **Direct-to-Final Writing**: Writing directly to the target filename. *Rejected:* Risk of users opening incomplete/corrupt files.
- **Lazy Allocation**: Allocating space as data arrives. *Rejected:* Risk of disk exhaustion mid-download and higher fragmentation.

## Consequences
- **Pros**: Guaranteed disk space, clean user folders (no partial files until ready), and robust resumption.
- **Cons**: Pre-allocation can be slow on some file systems (like FAT32 or some network drives) if not supported natively. We may need an option to disable it.
