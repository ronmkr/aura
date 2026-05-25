---
title      : "Feat: Add FTPS (TLS) support and retry logic to FTP worker"
labels     : [type:enhancement, priority:moderate, area:worker-ftp]
description: |
  The FTP worker (`worker/ftp.rs`) has two gaps identified in the deep-dive audit:
  1. **No FTPS/TLS**: The `suppaftp` crate supports TLS via `into_secure()`, but this is never called. All FTP connections are plaintext.
  2. **No retry logic**: Unlike the HTTP worker (which retries with exponential backoff on 5xx/429/network errors), the FTP worker makes a single attempt — any error is terminal.

  Acceptance criteria:
  - Attempt TLS upgrade via `into_secure()` when connecting to `ftps://` URIs or when the server supports AUTH TLS.
  - Implement retry logic matching the HTTP worker pattern (configurable retry count + exponential backoff).
  - Add integration test for FTP retry on connection failure.
---
