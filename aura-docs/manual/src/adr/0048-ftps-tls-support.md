# Decision 0048: FTPS (TLS) Support and Retry Logic

Date: 2026-05-27

## Status

Implemented (2026-05-28, PR #133); refactored to `rustls-ring` (2026-06-04, Issue #189)

## Context

The FTP protocol relies on plain-text communication, which leaves data and credentials vulnerable to interception. While Decision-0018 introduced FTP support, it did not strictly mandate TLS wrapping for secure environments. To maintain parity with modern download managers, and to adhere to our security-first design principles, we need native support for FTPS (FTP over TLS/SSL). Additionally, FTP servers often aggressively rate-limit or drop connections, necessitating a robust retry logic within the worker.

## Decision

We will extend the FTP worker to support Explicit FTPS (AUTH TLS).
- If the URI scheme is `ftps://`, the worker will automatically negotiate a TLS session after the initial TCP connection using `rustls` with the `ring` crypto provider.
- Plain FTP connections that advertise `AUTH TLS` via FEAT will be opportunistically upgraded to TLS.
- We will integrate a robust retry loop for the FTP worker. Transient failures (e.g., connection reset, 421 Service not available) will be caught, and the worker will back off exponentially before re-attempting to download the assigned chunk.
- The `rustls` configuration will use `rustls-native-certs` to load the OS certificate store, consistent with Decision-0014 and Decision-0028.
- The `native-tls` dependency is removed entirely. All TLS in the project is unified under `rustls-ring`.

## Consequences

- **Pros:** Eliminates the `native-tls`/`OpenSSL` dependency, simplifying the build. Provides a uniform TLS backend across all protocols. Enhances security for users downloading from legacy FTP hosts that offer TLS. Increases download reliability on unstable FTP servers.
- **Cons:** Slightly increases the complexity of the FTP worker state machine (handling TLS handshake timeouts). Servers with non-standard or expired certificates may fail where `native-tls` was more permissive.

## Implementation
- **FTPS & Retry Logic**: Implemented in `aura-core/src/worker/ftp.rs` (2026-05-28, PR #133).
- **rustls-ring migration**: Refactored to `suppaftp` `tokio-rustls-ring` feature, removing `native-tls` (2026-06-04, Issue #189).
