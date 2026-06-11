# ADR 0069: Watch Folder Auto-ingestion

## Status
Proposed (2026-06-11 — Issue #288)

## Context
Aura relies on external triggers (CLI inputs, RPC requests, browser extensions) to add downloads. There is no file-system-driven ingestion mechanism. Power users and server environments expect a "Watch Folder" workflow: dropping a `.torrent`, `.metalink`, or `.meta4` file into a monitored directory should automatically schedule the download. Aura already depends on the `notify` crate and uses it in `aura-core/src/orchestrator/engine.rs` to hot-reload `Aura.toml`. The same filesystem monitoring infrastructure can be leveraged to support a watch folder.

## Decision
1. Add a `[storage] watch_dir = "/path/to/watch"` option to `Aura.toml` (disabled by default).
2. Instantiate a second `notify` watcher in `aura-core/src/orchestrator/engine.rs` targeting the configured `watch_dir`.
3. Monitor for `Create` and `Write` file events. If a file ending in `.torrent`, `.metalink`, `.meta4`, or `.nzb` is created, trigger ingestion:
   - Wait for the file write operation to complete (debounce logic).
   - Parse the file and call `Engine::add()` internally to create the download task.
4. On successful addition, move the source file to a `watch_dir/processed/` subdirectory to avoid infinite loops and maintain directory cleanliness.
5. If parsing/ingestion fails, move the source file to `watch_dir/failed/` and log a structured warning.
6. Expose the watch folder status and last-ingested file path via `aura status` and the TUI dashboard.

## Edge Cases
1. **Partial Writes**: Browsers and download managers write files to disk incrementally. Triggering ingestion too early leads to parse errors. Implement a debounce mechanism (e.g., checking if the file size remains unchanged for 1 second, or waiting for a rename/close event if supported by the OS).
2. **Duplicate Ingestion**: If the same file is dropped repeatedly, the Engine must return a duplicate task error gracefully and avoid spinning up redundant downloads.
3. **Failing to Move Files**: If Aura does not have write permissions to delete or move the file after ingestion, log an error and disable the watcher to prevent infinite ingestion loops.

## Alternatives Considered
- **Cron Polling**: Periodically poll the watch folder directory using a background thread (e.g., every 10 seconds). *Rejected:* Inefficient; introduces latency; notify provides instant, event-driven detection.
- **Client-Side Watcher**: Force the CLI client to watch a folder and send RPC commands. *Rejected:* Daemon must run independently of terminal sessions; CLI client is not always active.

## Consequences
- **Pros**: Zero-friction automated ingestion for web browsers and file sharing; leverages existing dependencies (`notify`).
- **Cons**: Adds directory file permission management requirements; introduces cross-platform file write event debouncing complexity.
