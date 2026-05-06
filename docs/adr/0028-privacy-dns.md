# ADR 0028: Privacy-Enhanced Resolution and Modern DNS

## Status
Accepted

## Context
Standard system DNS resolution can be a bottleneck and is often subject to ISP filtering or lack of privacy. Modern tools (`curl`, browsers) provide asynchronous resolution and **DNS over HTTPS (DoH)** to address these issues.

## Decision
1. **Async DNS**: We will use the `trust-dns-resolver` or `hickory-resolver` crate to perform non-blocking DNS resolution, allowing multiple parallel connection attempts without blocking thread pools.
2. **DNS over HTTPS (DoH)**: The system will support DoH providers (e.g., Cloudflare, Google) to ensure privacy and bypass local network restrictions.
3. **DNS Caching**: We will implement a thread-safe local DNS cache within the **Privacy-Enhanced Resolver** to reduce redundant network round-trips.
4. **Happy Eyeballs Integration**: The resolver will provide both IPv4 and IPv6 addresses simultaneously to support the **Happy Eyeballs** connection strategy.

## Alternatives Considered
- **System Resolver (`getaddrinfo`)**: *Rejected:* Blocking by nature; requires a separate thread pool and lacks DoH support.

## Consequences
- **Pros**: Lower latency, enhanced privacy, and better reliability on restrictive networks.
- **Cons**: Increases the dependency count and complexity of the networking stack.
