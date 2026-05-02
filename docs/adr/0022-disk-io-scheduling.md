# ADR 0022: Advanced Disk I/O Scheduling and Kernel Hinting

## Status
Accepted

## Context
Standard file I/O can be a bottleneck for high-speed downloads and can degrade system performance by polluting the OS page cache with temporary download data. `aria2` addresses this through write caching, `mmap`, and `posix_fadvise`.

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
