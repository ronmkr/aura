# Decision 0060: Pre-Download Disk Space Verification

## Status

Implemented (2026-06-04, PR #259 — Issue #242)

## Context

Before pre-allocating a `.part` file for a large download, the Storage Engine (Decision-0002, Decision-0003) must verify that the target filesystem has sufficient free space. Without this check, `fallocate()` or sequential zero-fill will fail partway through, leaving a corrupt partial file that (a) consumes as much disk space as was successfully allocated, (b) cannot be safely resumed since the actual file size may be less than `total_length`, and (c) provides no actionable error message to the user. A BDD scenario documenting this behavior exists in `aura-core/tests/steps/errors.rs` but the `given_drive_free_space` and `then_fail_preallocation` step implementations are empty stubs — the test always passes regardless of actual disk conditions. Additionally, for streaming mode downloads where `total_length` is unknown until completion, a dynamic high-watermark check must be applied during download. Related: GitHub Issue #242.

## Decision

1. Before any pre-allocation call in `storage/registry.rs`, query the available filesystem space using `statvfs()` on Unix and `GetDiskFreeSpaceExW()` on Windows via the `fs2` crate or equivalent.
2. Require a minimum headroom: `available_bytes > total_length + (total_length * 0.05).max(512 * 1024 * 1024)` (i.e., 5% or 512 MB extra, whichever is larger). Return `StorageError::InsufficientDiskSpace { needed: u64, available: u64 }` on failure.
3. For streaming mode or unknown-length downloads, implement a periodic high-watermark check: every 256 MB written, re-check available space and pause the task with `DownloadPhase::Paused(PauseReason::InsufficientDiskSpace)` if headroom drops below 1 GB.
4. Surface the `InsufficientDiskSpace` error through the event bus (Decision-0004) so the TUI and CLI can display a clear user-facing message with both the needed and available space.
5. Implement the BDD step stubs in `aura-core/tests/steps/errors.rs` using filesystem-level injection (a mock `DiskSpaceProber` trait injected into the Storage Engine for tests).

## Edge Cases

1. **TOCTOU Race**: Between the space check and the `fallocate()` call, another process may consume available space. Mitigation: catch `ENOSPC` errors from `fallocate()` and map them to `InsufficientDiskSpace` with a retry hint.
2. **Quota-limited users**: On systems with per-user disk quotas (Linux `quota`, macOS sandbox quotas), `statvfs.f_bavail` may be larger than the user's actual quota. Mitigation: catch `EDQUOT` from write syscalls and surface as `InsufficientDiskSpace`.
3. **Multiple simultaneous tasks**: If three 10 GB tasks start concurrently and 25 GB is available, each individual check passes but combined they exceed capacity. Mitigation: the Storage Engine must maintain a `reserved_bytes: AtomicU64` counter tracking space committed by in-progress pre-allocations but not yet consumed.
4. **Copy-on-Write (COW) filesystems**: On Btrfs, ZFS, or APFS, `statvfs` may not accurately reflect space available after deduplication/compression. Warn the user when a COW filesystem is detected that space estimates may be approximate.

## Alternatives Considered

- **Only handle ENOSPC errors**: Let pre-allocation fail and handle the OS error. *Rejected:* Provides poor UX — the error only surfaces after potentially corrupt partial writes; also, `fallocate()` on some filesystems (ext3, NFS) silently succeeds then fails on actual write.
- **Let the Allocation Prober (Decision-0052) handle it**: Use the existing prober. *Rejected:* The prober checks if the filesystem supports sparse files, not if space is available. Different concern.

## Consequences

- **Pros**: Prevents silent disk-full corruption; gives users actionable error messages before wasting time on a doomed download; enables clean pause/resume when space becomes available later.
- **Cons**: Adds 1–2 syscalls per task creation; COW filesystem estimates are imprecise and require a disclaimer.
