# Decision 0022: Advanced Disk I/O Scheduling and Kernel Hinting

## Status

Partially Implemented (Audit 2026-06-03)

## Context

Standard file I/O can be a bottleneck for high-speed downloads and can degrade system performance by polluting the OS page cache with temporary download data. Aura addresses this through write caching, `mmap`, and `posix_fadvise`.

## Decision

1. **Disk I/O Scheduler**: We will implement a scheduler within the **Storage Engine** that uses **Async I/O** (e.g., `io-uring` on Linux via `tokio-uring`) as the preferred path. This enables non-blocking, zero-copy writes.
2. **FADV Strategy**: The engine will automatically apply kernel hints:
  - `POSIX_FADV_DONTNEED`: Applied after a piece is successfully verified and flushed to disk, suggesting the OS can drop it from RAM.
  - `POSIX_FADV_SEQUENTIAL`: Applied for single-stream downloads (HTTP/FTP) to trigger aggressive read-ahead.
3. **Mmap Fallback**: For platforms without robust async I/O support (or for specific user requests), the engine will fall back to **Memory Mapped I/O** for large files to reduce the overhead of repeated `write` syscalls.
4. **Sparse Allocation**: On supported filesystems, the engine will use `FSCTL_SET_SPARSE` (Windows) or hole-based writes (POSIX) to initialize downloads instantly.

## Alternatives Considered

- **Standard Tokio FS**: Using `tokio::fs`. *Rejected:* `tokio::fs` uses a thread pool for blocking I/O, which is less efficient than native async I/O like `io-uring` for high-throughput downloads.
- **Direct I/O Only**: Always using `O_DIRECT`. *Rejected:* Requires complex alignment management and can actually be slower for some filesystems/protocols that benefit from the page cache.

## Consequences

- **Pros**: Minimal CPU overhead for I/O, prevention of system-wide performance degradation during large downloads, and optimized performance for BitTorrent's random-access writes.
- **Cons**: `io-uring` is Linux-specific; maintaining platform-specific I/O paths (io-uring vs. mmap vs. thread-pool) increases the complexity of the **Storage Engine**.

## Implementation

- **Disk I/O Scheduler & Hinting**: Initially scoped in `aura-core/src/storage/ops.rs` and `aura-core/src/storage/scheduler.rs` (2026-05-30, PR #163).

## Implementation Status

- **Kernel Hinting (posix_fadvise)**: Fully implemented.
- **Disk I/O Scheduler (io_uring)**: Pending implementation behind a linux-specific feature flag (tracked in Issue #295).
- **Mmap Fallback**: Pending implementation for large file operations (tracked in Issue #295).
