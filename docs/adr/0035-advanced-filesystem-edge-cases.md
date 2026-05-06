# ADR 0035: Advanced Filesystem Edge Cases (COW, Long Paths, Endgame)

## Status
Accepted

## Context
Filesystems behave drastically differently under load (e.g., ZFS vs. Ext4 vs. NTFS). Furthermore, OS path limits (Windows 260 chars) and BitTorrent "last piece" mechanics can silently destroy performance or corrupt downloads.

## Decision
1. **COW-Aware Allocator**: The **Filesystem Adapter** will probe the destination path. If a Copy-On-Write filesystem (Btrfs, ZFS) is detected, standard `fallocate` will be disabled. Instead, the engine will attempt to apply the "No-COW" attribute (`chattr +C` on Linux) to prevent catastrophic physical fragmentation caused by random BitTorrent writes.
2. **Path Truncator**: The **Path Normalizer** will automatically prefix long paths with `\\?\` on Windows. If a generated path still exceeds OS limits or contains invalid characters, it will safely truncate directory or file names while preserving the extension.
3. **Endgame Broadcaster**: To prevent "99% stalls" in BitTorrent, the **Piece Selector** will enter Endgame Mode when the final few blocks are required. It will broadcast requests to all active peers holding those blocks simultaneously, canceling the duplicate requests once the data is received.

## Alternatives Considered
- **Ignore COW fragmentation**: *Rejected:* Can lead to performance degradation that takes hours to fix via defragmentation tools.
- **Fail on Long Paths**: *Rejected:* Users have no control over the directory depth defined inside a `.torrent` metadata file.

## Consequences
- **Pros**: Bulletproof reliability on complex filesystems and OSs, and mathematical prevention of the "last-piece stall" in P2P downloads.
- **Cons**: Detecting filesystem types dynamically (e.g., checking if a path is on a Btrfs mount) is highly OS-specific and complex in Rust.
