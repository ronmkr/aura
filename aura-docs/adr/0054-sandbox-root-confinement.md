# ADR 0054: SandboxRoot Confinement for Storage Engine

## Status
Implemented (2026-06-03, remediation/immediate-security)

## Context
Malicious or malformed torrents, Metalinks, or HTTP files can contain filenames with relative directory components (e.g., `../../etc/passwd` or `../../.ssh/authorized_keys`). If not properly confined, the Storage Engine could overwrite or read files outside the designated download folder, leading to path traversal vulnerability and potential remote code execution or file system leakage.

Currently, Aura prevents basic `..` inclusions in filenames in `mapping.rs`, but there is no absolute boundary enforcement at the virtual filesystem layer inside the `StorageEngine` (GAP-02).

## Decision
1. **Configurable Sandbox Root**: Introduce a `sandbox_root` parameter in `Aura.toml` under a security/storage section, defaulting to the absolute path of the download directory.
2. **Path Canonicalization & Verification**: Implement a `SandboxRoot` helper module within `aura-core/src/storage/`. This module will canonicalize all targets before any file operations (open, create, read, write) occur.
3. **Strict Validation**: For every file operation:
   - Resolve the target file path to its absolute, canonical path.
   - Verify that this canonical path starts with the prefix of the canonical `sandbox_root`.
   - Reject any operation where the target path escapes the `sandbox_root` boundary with an explicit `StorageError::PathTraversal` error.

## Edge Cases
1. **Symlink and Hardlink Resolution**: If a torrent contains a symlink, the Storage Engine must canonicalize the link target. If it points outside the `sandbox_root`, the link creation or write must be rejected. To minimize risks, symlinks will be resolved to their real paths before checking, and nested symlinks will be restricted.
2. **Case-Insensitive Filesystems**: On macOS (APFS case-insensitive) and Windows (NTFS), path comparison is vulnerable to case variations (e.g. `/Downloads/Aura` vs `/downloads/aura`). Canonicalization via `std::fs::canonicalize` will be used to resolve the actual filesystem casing before comparison.
3. **Absolute Payload Paths**: If a torrent file metadata declares an absolute path (e.g., `/etc/passwd`), it must be treated as relative to the `sandbox_root` (concatenated) rather than being resolved from the system root, and then validated.
4. **Special Device Files**: Attempts to write to system reserved names (e.g., `/dev/sda` on Linux or `CON`, `PRN`, `NUL` on Windows) will be explicitly checked and rejected to prevent system disruption.

## Alternatives Considered
- **Filename Sanitization Only**: Disallowing `..` in all file inputs. *Rejected:* Incomplete and easily bypassed with complex path encodings or symlinks. Canonicalization + prefix verification is the industry standard for path traversal prevention.
- **System-level Containerization (chroot/Docker)**: Relying solely on Docker or system containers. *Rejected:* Aura must run securely as a native process without requiring root/containers.

## Consequences
- **Pros**: Robust protection against path traversal attacks; clear security boundary for Storage Engine.
- **Cons**: Resolving and canonicalizing paths adds minor filesystem lookup latency on file open/create operations.
