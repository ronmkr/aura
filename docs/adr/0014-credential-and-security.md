# ADR 0014: Credential and Security Abstraction

## Status
Accepted

## Context
`aria2` handles secrets from diverse sources (`netrc`, cookies, command-line) and must negotiate secure connections across different operating systems with varying TLS implementations.

## Decision
1. **Credential Provider**: We will implement a centralized resolver that aggregates authentication data. Components only request "credentials for URL X" rather than knowing *where* those credentials came from.
2. **Security Context**: We will use the `rustls` crate for most TLS operations but provide an abstraction layer to support platform-native backends (via `native-tls`) for environments that require Apple TLS or WinTLS integration.
3. **Cookie Lifecycle**: Cookies will be managed in a thread-safe **Cookie Storage** that persists to disk using the standard Mozilla/Netscape format, ensuring compatibility with other tools.

## Alternatives Considered
- **Direct Library Usage**: Using `reqwest` or `hyper` defaults. *Rejected:* Doesn't allow for the fine-grained control over credential resolution (like `netrc` priority) required for `aria2` parity.

## Consequences
- **Pros**: Clean security boundaries, cross-platform compatibility, and full parity with `aria2` authentication workflows.
- **Cons**: Managing multiple TLS backends adds complexity to the build system.
