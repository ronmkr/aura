Status: Proposed

# ADR 0061: Checksum Verification for HTTP and FTP Downloads

## Status
Proposed

## Context
The `TaskState` and `AddTaskArgs` structs both include a `checksum: Option<Checksum>` field (ADR-0041 covers integrity verification for BitTorrent via piece hashes). However, for HTTP and FTP downloads, the checksum field is silently ignored: `aura-daemon/src/jsonrpc.rs` hardcodes `checksum: None` when creating RPC tasks, and `aura-core/src/orchestrator/event_handlers.rs` also hardcodes `checksum: None` at all HTTP completion events. The `ScrubberActor` in `aura-core/src/scrubber/` handles BitTorrent integrity but has no equivalent invocation for HTTP/FTP task completion. This means users who pass a SHA-256 or MD5 hash for an HTTP download (e.g., downloading an OS ISO with a published hash) receive no verification — a corrupted or tampered download is indistinguishable from a valid one. Related: GitHub Issue #243.

## Decision
1. Extend the `aria2.addUri` RPC handler to accept an optional `checksum` parameter in the options object: `{"checksum": "sha-256=abc123..."}` following the aria2 checksum format (`algorithm=hexdigest`).
2. Wire the checksum through `AddTaskArgs.checksum` and store it in `TaskState.checksum` (already persisted).
3. After an HTTP or FTP download completes and the `.part` file is finalized (immediately before the rename to the final filename), invoke the `ScrubberActor` or an equivalent `verify_file_checksum(path, checksum)` function.
4. Support SHA-256, SHA-1, and MD5 hash algorithms (with a deprecation warning for MD5 and SHA-1 due to collision vulnerabilities).
5. On checksum mismatch: emit a `TaskEvent::IntegrityFailure { task_id, expected, actual }` event; delete the corrupted `.part` file; mark the task as `DownloadPhase::Error` with `ErrorKind::IntegrityFailure`; optionally retry from a different mirror source if available.
6. On checksum match: emit a `TaskEvent::IntegrityVerified { task_id }` event; proceed with rename to final filename.
7. Metalink manifests (ADR-0013) provide per-piece and whole-file checksums — these must also be wired through the same verification pipeline when a Metalink source is used.

## Edge Cases
1. **Streaming Mode**: When `streaming_mode = true`, the file is consumed by the media player as it arrives. Post-download hash verification of a streamed file is impractical. For streaming mode tasks with a checksum, perform best-effort streaming verification using an incremental hasher (e.g., `sha2::Sha256`) that digests each written segment in real time.
2. **Multi-Mirror Downloads**: Data segments come from multiple sources (Mirror A for bytes 0–512 MB, Mirror B for 512 MB–1 GB). The whole-file hash must be computed over the assembled file after all segments are written and merged, not per-segment.
3. **Checksum of Compressed Content**: Some servers serve pre-compressed content with `Content-Encoding: gzip` transparently decoded by reqwest. The user's checksum may refer to the compressed or decompressed form. Document which form is verified (decompressed, as written to disk).
4. **Hash Format Errors**: User passes `{"checksum": "invalid-string"}`. Return a `UriValidationError::MalformedChecksum` before the task is created.

## Alternatives Considered
- **Trust the network**: Rely on TLS to prevent tampering. *Rejected:* TLS only prevents in-transit tampering; it does not protect against server-side corruption, CDN caching bugs, or intentional content swaps.
- **Mandatory checksum**: Require a checksum for all downloads. *Rejected:* Most casual downloads don't have published checksums; a mandatory requirement would break common usage.

## Consequences
- **Pros**: Users can verify ISO integrity, software authenticity, and dataset completeness without running external tools; enables automatic retry on corruption.
- **Cons**: Full-file hash computation adds 1–3 seconds for multi-gigabyte files (IO-bound, but noticeable); streaming verification adds ~5% CPU overhead.
