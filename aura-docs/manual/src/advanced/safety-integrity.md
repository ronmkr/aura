# Safety & Data Integrity

Aura is a safety-critical download engine. Beyond high-speed throughput, it implements multiple layers of protection to ensure that every byte written to disk is exactly what was intended by the source.

## 1. Atomic Completion (Decision 0003 & 0035)

To prevent partially downloaded or corrupted files from appearing as "complete" to the OS or other applications, Aura uses an atomic transition model:

- **`.part` Extensions**: All active downloads are written to temporary files with a `.part` extension.
- **Pre-allocation (`fallocate`)**: Before writing data, Aura reserves the full file size on the filesystem. This guarantees that the download won't fail mid-way due to lack of space and reduces fragmentation.
- **`fsync` Durability**: Before renaming a file, Aura performs an `fsync` or `fdatasync` (Issue #117). This ensures all OS write buffers are physically flushed to the disk platter or flash cells.
- **Atomic Rename**: Only after a successful flush is the `.part` extension removed. On most filesystems, this is an atomic operation, meaning you never see a partially-named file.

## 2. Pre-Download Disk Space Verification (Decision 0060)

Before pre-allocating a large download, the Storage Engine verifies that the target filesystem has sufficient free space.

- **Headroom Requirement**: Aura requires available space equal to the `total_length` plus a safety margin: `5%` of the file size or `512 MB`, whichever is larger.
- **Streaming Mode Protection**: For downloads with unknown lengths, Aura performs a dynamic high-watermark check every 256 MB. If the available space drops below 1 GB, the task is automatically paused with an `InsufficientDiskSpace` error.
- **Quota Awareness**: Aura attempts to detect user disk quotas (`EDQUOT`) and surfaces them as actionable space errors rather than generic I/O failures.

## 3. Integrity Scrubber (Decision 0024)

The **Integrity Scrubber** is a background actor that proactively heals downloads:
- **Background Verification**: It periodically scans the bitfields of active tasks and re-verifies random pieces against known hashes.
- **Self-Healing**: If corruption is detected (e.g., due to a dying disk sector), the scrubber marks the piece as "missing" in the bitfield and dispatches a `RefreshDiscovery` event to the Orchestrator to re-download the piece from the swarm.

## 4. Fast Resume and Piece Recheck (Decision 0068)

At task startup, Aura automatically scans for existing target or part files to resume the download safely and quickly:
- **BitTorrent / Multi-Source**: Aura initiates a background hash validation process to verify which pieces are already present on disk, updating the download bitfield to ensure only missing or corrupted pieces are fetched from the network.
- **HTTP / FTP**: Aura inspects the size of the existing `.part` file to establish the correct range request byte boundaries.
- **On-Demand Recheck**: A full integrity scan can be manually forced at any time via the `"aura.forceRecheck"` JSON-RPC method or the `aura recheck <GID>` CLI command.
- **TUI & Progress Feedback**: Progress of the integrity scan is exposed to RPC clients as `recheck_progress` (from `0.0` to `1.0`) and displayed on the TUI dashboard as an active "Rechecking" gauge to prevent blocking user interaction during verification.

## 5. Raii Piece Guards (issue #161)

To prevent "Zombie Pieces" (pieces marked as in-progress that are never finished due to a worker crash), Aura uses **Resource Acquisition Is Initialization (RAII)**:
- **`PieceGuard`**: When the `PiecePicker` assigns a piece to a worker, it returns a `PieceGuard` object.
- **Automatic Release**: If the worker's asynchronous task is aborted (network drop, panic, timeout), the `PieceGuard` is dropped, and its `Drop` implementation automatically releases the piece back to the picker for reassignment.

## 6. Non-Swarm Checksum Verification (Decision 0041, 0061)

Unlike BitTorrent, standard HTTP/FTP downloads don't have built-in hashing. Aura adds this capability:
- **CLI & RPC Support**: Use `--checksum=sha256:HASH` or provide a `checksum` parameter in the `aura.addUri` RPC call.
- **Supported Algorithms**: SHA-256 (recommended), SHA-1, and MD5 (deprecated).
- **Streaming Verification**: For streamed downloads, Aura uses an incremental hasher to digest segments in real-time as they are written to disk.
- **Post-Download Scrub**: Once 100% of the bytes are retrieved, the engine transitions to a `Verifying` phase. It hashes the entire file and only moves to `Complete` if the checksum matches. If it fails, the `.part` file is deleted to prevent accidental use of corrupted data.

## 7. Merkle Tree Block Verification (bep 52)

In **BitTorrent v2**, Aura doesn't wait for a 16MB piece to finish before checking integrity.
- **16KB Hashing**: It verifies each 16KB block against the SHA-256 Merkle tree as soon as it arrives.
- **Immediate Rejection**: Malicious or corrupted blocks from "poison peers" are detected and dropped within microseconds, saving bandwidth and preventing swarm pollution.

## 8. SandboxRoot Confinement (Decision 0054)

To protect against path traversal attacks (e.g., malicious torrents containing `../../etc/passwd`), Aura's Storage Engine implements absolute boundary enforcement:
- **Canonicalization**: All file paths are resolved to their absolute, canonical paths before any file operation (open, create, read, write).
- **Boundary Verification**: Aura strictly verifies that the resulting canonical path is a child of the defined `sandbox_root` (which defaults to the download directory).
- **Rejection**: Any operation attempting to escape the sandbox is rejected with a `StorageError::PathTraversal` error.

## 9. SecretScrubber Log Sanitization (Decision 0055)

Aura ensures that sensitive authentication credentials are not leaked during debugging or production monitoring:
- **Tracing Interception**: A centralized `SecretScrubber` intercepts all logs and structured telemetry spans.
- **Pattern Redaction**: It uses pre-compiled patterns to identify and redact sensitive information like `Authorization: Bearer` tokens, Basic Auth credentials, cookies, and inline URL credentials (e.g., `http://user:pass@host`) before they are written to disk or the console.

## 10. URI Validation & SSRF Mitigation (Decision 0059)

Aura protects systems from Server-Side Request Forgery (SSRF) when accepting URIs via its RPC endpoints:
- **Scheme Allowlist**: Only safe schemes (`http://`, `https://`, `ftp://`, `ftps://`, `magnet:`) are permitted. Dangerous schemes like `file://` are strictly rejected.
- **Private IP Blocking**: Aura resolves hostnames and blocks connections to private networks (RFC 1918), loopback, and link-local addresses, preventing attackers from scanning or exfiltrating data from internal infrastructure (like cloud metadata endpoints).
- **Intranet Exemptions**: Legitimate local downloads (e.g., from a home NAS) can be explicitly authorized via the `allowed_private_ranges` setting in `Aura.toml`.
