# Network Filesystems (nfs & Smb)

Aura includes specialized optimizations for downloading to network-attached storage (NAS) and shared drives. Downloading to these environments presents challenges like high latency and limited support for standard filesystem features like `fallocate`.

## 1. Automatic NAS Detection (Decision 0021)

The **Storage Engine** automatically identifies when a download path resides on a network share (NFS or SMB).

- **Detection Mechanism**: Aura uses platform-specific syscalls to inspect the filesystem type of the target directory.
- **Adaptive Strategy**: If a network share is detected, Aura automatically switches its pre-allocation strategy to **Lazy Allocation** if native `fallocate` is unsupported or slow on the mount.

## 2. Latency Masking & Write Aggregation

Network filesystems are highly sensitive to small, random writes. Aura mitigates this by:
- **Increased Flush Thresholds**: When on a network share, Aura automatically increases the amount of data buffered in RAM (up to the limits defined by the [Resource Governor](./resource-governor.md)) before performing a flush.
- **Sequential Priority**: The engine prioritizes flushing contiguous blocks of data, reducing the number of round-trip "write" commands sent to the NAS.

## 3. Advisory File Locking

To prevent data corruption in multi-user environments where multiple Aura instances or other applications might access the same share:
- **`flock` Integration**: Aura attempts to acquire an advisory lock on the download file.
- **Conflict Prevention**: If another process has locked the file, Aura will pause the task and alert the user with a `FileLocked` error, preventing two instances from overwriting each other's data.

## 4. Best Practices For Nas

For the best performance on NFS/SMB:
- **Use Wired Ethernet**: Avoid downloading directly to a NAS over Wi-Fi, as the combined latency of the internet and the local network can lead to buffer exhaustion.
- **Tune `io_deadline_ms`**: In high-latency environments, you may need to increase the `io_deadline_ms` in `Aura.toml` to prevent the Storage Engine from timing out during large flushes.
