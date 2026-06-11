# ADR 0068: Fast Resume and Piece Recheck

## Status
Proposed (2026-06-11 — Issue #284)

## Context
When a download task is added or the daemon restarts, Aura currently ignores any existing data at the target file paths and starts downloading from byte 0. In NAS, home server, and seedbox environments, daemon restarts are common due to updates, reboots, or power failures. Re-downloading massive files from scratch on every restart wastes significant network bandwidth and storage I/O, and causes unacceptable delay. While Aura implements metadata persistence (ADR-0017) and atomic renaming (ADR-0003), it lacks the ability to verify and resume downloads from existing partial files on disk.

## Decision
1. Implement a Fast Resume and Piece Recheck mechanism in `aura-core/src/storage/recheck.rs`.
2. When a task starts, check if a target file or a `.part` file already exists. If yes, trigger a background hash verification phase.
3. Compute the cryptographic hash of each local piece against the torrent's piece hashes (or segment checksums for HTTP/FTP downloads).
4. Populate the task's initial download bitfield with the verified pieces, skipping network downloads for verified blocks.
5. Perform this recheck concurrently with active network downloads for missing pieces to prevent thread blocking.
6. Expose the recheck progress (`0.0..1.0`) via the JSON-RPC interface (`tellStatus` method) and display it on the TUI dashboard.
7. Provide a CLI subcommand: `aura recheck <GID>` to manually trigger a full integrity scan of any task's data.

## Edge Cases
1. **Corrupted Data**: If a piece fails the hash check, mark it as missing/dirty in the bitfield so the network engine re-downloads it. Do not discard the entire file.
2. **File Size Mismatches**: If the existing file on disk is larger or smaller than the expected size, handle it gracefully by either truncating/extending the file or treating it as a corrupt file based on the segment structure.
3. **High Disk I/O during Recheck**: Hashing a multi-gigabyte file can saturate disk I/O, starving other tasks. The rechecking thread must yield or throttle its read throughput if high I/O latency is detected.

## Alternatives Considered
- **Strict Session Saving**: Rely entirely on saving the last active bitfield state to the database on shutdown. *Rejected:* If the daemon crashes unexpectedly, the DB state could be out of sync with actual disk data. Furthermore, this does not support re-adding tasks with existing files downloaded outside of Aura.
- **Force Re-download**: Force users to manually handle resuming. *Rejected:* Destroys usability in server/daemon deployments.

## Consequences
- **Pros**: Prevents redundant downloads on restart; ensures data integrity by verifying existing files; improves TUI usability during task startup.
- **Cons**: Adds initial startup disk read overhead; requires managing transition states (Rechecking -> Downloading) in the Orchestrator.
