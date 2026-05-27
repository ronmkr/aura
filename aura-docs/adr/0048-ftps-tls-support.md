# 48. FTPS (TLS) Support and Retry Logic

Date: 2026-05-27

## Status

Accepted

## Context

The FTP protocol relies on plain-text communication, which leaves data and credentials vulnerable to interception. While ADR-0018 introduced FTP support, it did not strictly mandate TLS wrapping for secure environments. To maintain parity with modern download managers (like `wget` and `aria2`), and to adhere to our security-first design principles, we need native support for FTPS (FTP over TLS/SSL). Additionally, FTP servers often aggressively rate-limit or drop connections, necessitating a robust retry logic within the worker.

## Decision

We will extend the FTP worker to support Explicit FTPS (AUTH TLS). 
- If the URI scheme is `ftps://`, the worker will automatically negotiate a TLS session after the initial TCP connection using `rustls`.
- We will integrate a robust retry loop for the FTP worker. Transient failures (e.g., connection reset, 421 Service not available) will be caught, and the worker will back off exponentially before re-attempting to download the assigned chunk.
- The `rustls` configuration will utilize the same certificate store built for HTTPS and DoH (ADR-0014, ADR-0028).

## Consequences

- **Pros:** Enhances security for users downloading from legacy FTP hosts that offer TLS. Increases download reliability on unstable FTP servers.
- **Cons:** Increases the complexity of the FTP worker state machine (handling TLS handshake timeouts).
