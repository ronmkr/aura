# Decision 0021: Network Filesystem Optimization (NFS/SMB)

## Status

Implemented (2026-06-04)

## Context

Downloading to network shares (NFS, SMB) presents unique challenges: high latency, potentially missing support for sparse files or `fallocate`, and risk of file corruption if multiple clients access the same share. Traditional download engines often experience performance degradation in these environments.

## Decision

1. **Filesystem Detection**: The **Storage Engine** will attempt to detect if a path is on a network share (using crates like `sysinfo` or platform-specific syscalls).
2. **Adaptive Pre-allocation**:
  - If native pre-allocation (e.g., `fallocate`) fails or is detected as unsupported, the engine will fall back to **Lazy Allocation** or a "Zeroing" pass that is optimized for network latency.
3. **Latency Masking**: For network shares, the **Buffer Pool** will automatically increase the "flush threshold" to aggregate more data in RAM before sending it over the network.
4. **File Locking**: We will implement advisory file locking (e.g., `flock`) to prevent multiple `Aura` instances from corrupting the same file on a shared mount.

## Implementation Status (2026-06-04)

- **Filesystem Detection & Pre-allocation**: Fully implemented via PR #196 (2026-06-02).
- **File Locking**: Advisory locking via cross-platform file locking (`std::fs::File::try_lock`) is fully implemented to prevent multiple Aura instances from corrupting the same file.

## Alternatives Considered

- **Universal Lazy Allocation**: Always use lazy allocation for simplicity. *Rejected:* Local disks benefit significantly from pre-allocation (less fragmentation).
- **Protocol-level SMB/NFS**: Implementing SMB/NFS as a **Protocol Worker**. *Rejected:* Too complex for Phase 1; users expect to download to a mounted directory.

## Consequences

- **Pros**: Robust performance on NAS/Home Server setups and prevention of common "Zero-size file" or "Disk full" errors on network shares.
- **Cons**: Detection can be platform-dependent and sometimes unreliable (e.g., in Docker or complex mount namespaces).

## Implementation Details

### AdvisoryLocker
The `AdvisoryLocker` component (`aura-core/src/storage/locker.rs`) manages file locks and network share detection for active tasks:
- **Locking Mechanism**: Utilizes the `fs2` crate's `try_lock` extension method on `std::fs::File` (retrieved via raw file descriptors/handles on Unix/Windows) to perform non-blocking advisory locking. This prevents multiple `Aura` processes from writing to the same destination path.
- **Network Share Detection**:
  - **Linux**: Calls `fstatfs` and matches the `f_type` magic numbers against known network filesystem identifiers: `NFS` (`0x6969`), `SMB` (`0x517B`), and `CIFS` (`0xFF534D42`).
  - **macOS**: Calls `fstatfs` and matches the `f_fstypename` string against `"nfs"`, `"smbfs"`, and `"afpfs"`.
  - **Other platforms**: Fallback defaults to local filesystem behavior.
- **Pre-allocation Bypass**: If a path is detected as residing on a network share, physical pre-allocation (`fallocate` or macOS `F_PREALLOCATE`) is automatically bypassed to avoid latency bottlenecks and failures on shares that do not support sparse blocks or pre-allocation.
