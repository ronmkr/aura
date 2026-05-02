# ADR 0034: Advanced Network Edge Cases (kTLS, Roaming, Captive Portals)

## Status
Accepted

## Context
Real-world networking involves hostile environments (Captive Portals), mobile users (Interface Roaming), and the need to maintain zero-copy performance even when traffic is encrypted (HTTPS).

## Decision
1. **Kernel TLS (kTLS)**: The **Zero-Copy Path** will attempt to negotiate `kTLS` for HTTPS streams on supported platforms (Linux/FreeBSD). This allows the kernel to decrypt the stream and pipe it directly to disk via `sendfile`, bypassing user-space RAM bottlenecks.
2. **Captive Portal Detector**: The engine will intercept initial HTTP redirects or HTML payloads when downloading non-HTML files (e.g., `.iso`, `.zip`). If a captive portal is detected, the task will be safely paused rather than saving the login page as corrupted data.
3. **Roaming Reconnector**: The **Orchestrator** will listen to netlink/OS routing events. If the active interface drops, it will pause **Protocol Workers** and attempt to resume them on the new default route automatically.

## Alternatives Considered
- **User-space TLS only**: *Rejected:* Limits maximum single-stream throughput on 10Gbps+ links due to CPU memory copying.
- **Fail-and-Retry for Roaming**: *Rejected:* Creates poor UX for laptop users moving between access points.

## Consequences
- **Pros**: Unmatched HTTPS performance, resilience to "Starbucks WiFi" corruption, and seamless mobility.
- **Cons**: `kTLS` requires specific kernel versions and OpenSSL/Rustls integrations which may complicate the build process.
