# ADR 0021: Network Filesystem Optimization (NFS/SMB)

## Status
Accepted

## Context
Downloading to network shares (NFS, SMB) presents unique challenges: high latency, potentially missing support for sparse files or `fallocate`, and risk of file corruption if multiple clients access the same share. `aria2` users often report performance issues in these environments.

## Decision
1. **Filesystem Detection**: The **Storage Engine** will attempt to detect if a path is on a network share (using crates like `sysinfo` or platform-specific syscalls).
2. **Adaptive Pre-allocation**: 
    - If native pre-allocation (e.g., `fallocate`) fails or is detected as unsupported, the engine will fall back to **Lazy Allocation** or a "Zeroing" pass that is optimized for network latency.
3. **Latency Masking**: For network shares, the **Buffer Pool** will automatically increase the "flush threshold" to aggregate more data in RAM before sending it over the network.
4. **File Locking**: We will implement advisory file locking (e.g., `flock`) to prevent multiple `Aura` instances from corrupting the same file on a shared mount.

## Alternatives Considered
- **Universal Lazy Allocation**: Always use lazy allocation for simplicity. *Rejected:* Local disks benefit significantly from pre-allocation (less fragmentation).
- **Protocol-level SMB/NFS**: Implementing SMB/NFS as a **Protocol Worker**. *Rejected:* Too complex for Phase 1; users expect to download to a mounted directory.

## Consequences
- **Pros**: Robust performance on NAS/Home Server setups and prevention of common "Zero-size file" or "Disk full" errors on network shares.
- **Cons**: Detection can be platform-dependent and sometimes unreliable (e.g., in Docker or complex mount namespaces).
