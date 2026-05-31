# Safety & Data Integrity

Aura is a safety-critical download engine. Beyond high-speed throughput, it implements multiple layers of protection to ensure that every byte written to disk is exactly what was intended by the source.

## 1. Atomic Completion (ADR 0003 & 0035)

To prevent partially downloaded or corrupted files from appearing as "complete" to the OS or other applications, Aura uses an atomic transition model:

- **`.part` Extensions**: All active downloads are written to temporary files with a `.part` extension.
- **Pre-allocation (`fallocate`)**: Before writing data, Aura reserves the full file size on the filesystem. This guarantees that the download won't fail mid-way due to lack of space and reduces fragmentation.
- **`fsync` Durability**: Before renaming a file, Aura performs an `fsync` or `fdatasync` (Issue #117). This ensures all OS write buffers are physically flushed to the disk platter or flash cells.
- **Atomic Rename**: Only after a successful flush is the `.part` extension removed. On most filesystems, this is an atomic operation, meaning you never see a partially-named file.

## 2. Integrity Scrubber (ADR 0024)

The **Integrity Scrubber** is a background actor that proactively heals downloads:
- **Background Verification**: It periodically scans the bitfields of active tasks and re-verifies random pieces against known hashes.
- **Self-Healing**: If corruption is detected (e.g., due to a dying disk sector), the scrubber marks the piece as "missing" in the bitfield and dispatches a `RefreshDiscovery` event to the Orchestrator to re-download the piece from the swarm.

## 3. RAII Piece Guards (Issue #161)

To prevent "Zombie Pieces" (pieces marked as in-progress that are never finished due to a worker crash), Aura uses **Resource Acquisition Is Initialization (RAII)**:
- **`PieceGuard`**: When the `PiecePicker` assigns a piece to a worker, it returns a `PieceGuard` object.
- **Automatic Release**: If the worker's asynchronous task is aborted (network drop, panic, timeout), the `PieceGuard` is dropped, and its `Drop` implementation automatically releases the piece back to the picker for reassignment.

## 4. Non-Swarm Checksum Verification (ADR 0041)

Unlike BitTorrent, standard HTTP/FTP downloads don't have built-in hashing. Aura adds this capability:
- **CLI Support**: Use `--checksum=sha256:HASH` or `--checksum=md5:HASH`.
- **Post-Download Scrub**: Once 100% of the bytes are retrieved, the engine transitions to a `Verifying` phase. It hashes the entire file and only moves to `Complete` if the checksum matches. If it fails, the file is preserved but marked as `Error: Checksum Mismatch`.

## 5. Merkle Tree Block Verification (BEP 52)

In **BitTorrent v2**, Aura doesn't wait for a 16MB piece to finish before checking integrity.
- **16KB Hashing**: It verifies each 16KB block against the SHA-256 Merkle tree as soon as it arrives.
- **Immediate Rejection**: Malicious or corrupted blocks from "poison peers" are detected and dropped within microseconds, saving bandwidth and preventing swarm pollution.

## 6. No-COW Awareness (Issue #70)

On modern Copy-on-Write (COW) filesystems like **Btrfs** and **ZFS**, random-access writes (like those in BitTorrent) can cause severe performance degradation and fragmentation.
- **Attribute Injection**: Aura detects these filesystems and automatically applies the `NOCOW` attribute (e.g., via `chattr +C` on Linux) to the download directory or file before pre-allocation begins.
