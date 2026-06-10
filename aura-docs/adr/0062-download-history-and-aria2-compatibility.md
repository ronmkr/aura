# ADR 0062: Download History Log and aura Protocol Compatibility

## Status
Implemented (2026-06-04, PR #259 — Issue #248)


## Context
Once a download task completes or is removed from the active queue, all information about it is lost — there is no persistent record of completed downloads. The `Engine::tell_active()` API only surfaces currently running tasks. The aura JSON-RPC protocol (which Aura emulates via ADR-0016) defines `aura.tellStopped` to list completed/removed downloads, `aura.getStatus` to query a task by GID, `aura.getVersion` for client identification, and `aura.saveSession` for explicit state persistence. None of these methods are implemented in `aura-daemon/src/jsonrpc.rs`. This incompatibility breaks aura-compatible UIs (AriaNg, webui-aura, Aria2App) which call `aura.getVersion` on connect and `aura.tellStopped` to display completed downloads history. The missing history also prevents users from auditing past downloads, verifying completion of scheduled jobs, or recovering from accidental file deletions. Related: GitHub Issue #244 (to be created).

## Decision
1. Introduce an append-only `~/.aura/history.jsonl` log file. On task completion, removal, or error, append a `CompletedTaskRecord { id: u64, name: String, uris: Vec<String>, total_bytes: u64, downloaded_bytes: u64, uploaded_bytes: u64, duration_secs: u64, checksum_verified: Option<bool>, phase: String, error: Option<String>, completed_at: DateTime<Utc> }` record as a single JSON line.
2. Implement `Engine::tell_history(offset: usize, num: usize) -> Vec<CompletedTaskRecord>` that reads and paginates the history log. The log is read-only after writing; no in-memory cache is maintained to avoid unbounded memory growth.
3. Implement the missing aura-compatible RPC methods: `aura.tellStopped(offset, num, keys)` — maps to `tell_history`; `aura.tellWaiting(offset, num, keys)` — returns queued but not yet started tasks; `aura.getStatus(gid, keys)` — queries both active and history; `aura.purgeDownloadResult()` — truncates the history log; `aura.removeDownloadResult(gid)` — removes a single record by GID; `aura.getVersion()` — returns `{"version": AURA_VERSION, "enabledFeatures": [...]}` (required for UI handshake); `aura.getSessionInfo()` — returns a session UUID; `aura.saveSession()` — explicit flush of active task states to `.aura` control files; `aura.shutdown()` — triggers graceful daemon shutdown (ADR-0058).
4. Add `aura history [--limit N] [--format json|table] [--filter failed|completed]` CLI subcommand.
5. Rotate the history log when it exceeds a configurable size limit (default: 10MB or 10,000 records, whichever is first). Keep the most recent N records, archive older records to `~/.aura/history.old.jsonl`.

## Edge Cases
1. **Concurrent Writes**: Multiple tasks completing simultaneously must not corrupt the history log. Use file-level advisory locking (consistent with ADR-0006) when appending, or use an actor-based history writer that serializes all writes.
2. **History Log Corruption**: If `history.jsonl` contains malformed lines (e.g., from a crash during write), the reader must skip malformed lines and log a warning rather than failing entirely.
3. **GID Reuse**: Task IDs are random `u64` values. Over time, the probability of collision is negligible but non-zero. The history reader must handle duplicate GIDs by returning the most recent entry.
4. **`aura.getStatus` for Completed Tasks**: aura clients expect `getStatus` to work even after a task is removed from the active queue. The implementation must query both `tell_active()` and the history log.
5. **History Filtering by Keys**: The aura protocol allows requesting specific fields via the `keys` parameter (e.g., `["gid", "status", "totalLength"]`). The response must filter to only the requested fields.

## Alternatives Considered
- **SQLite for history storage**: Use an embedded database for structured querying. *Rejected:* Adds a heavy dependency; JSONL is sufficient for the expected history size and provides better debuggability.
- **Keep completed tasks in memory**: Maintain a bounded in-memory ring buffer of last N completed tasks. *Rejected:* Does not survive daemon restarts; bounded ring loses older records.

## Consequences
- **Pros**: Full aura UI compatibility; auditable download history; enables `aura history` CLI; unblocks integration with AriaNg and webui-aura.
- **Cons**: Disk writes on task completion (negligible overhead); history log requires periodic maintenance (rotation).
