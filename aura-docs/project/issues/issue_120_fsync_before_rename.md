---
title      : "Bug: Add fsync/fdatasync before atomic .part rename to prevent data loss"
labels     : [type:bug, priority:critical, area:storage]
description: |
  The storage engine performs an atomic `.part` → final rename via `fs::rename()` in `storage/ops.rs`, but does NOT call `fsync()` or `fdatasync()` on the file descriptor before renaming. If the system crashes between the OS writing data to the page cache and the actual disk writeback, completed downloads could be silently corrupted or truncated.

  Discovered during the 2026-05-24 code-level deep-dive audit.

  Acceptance criteria:
  - Call `file.sync_all()` (or `sync_data()`) on the `.part` file handle before `fs::rename()`.
  - Call `fsync()` on the parent directory after rename (Linux best practice for metadata durability).
  - Add a unit test that verifies the sync call ordering.
---
