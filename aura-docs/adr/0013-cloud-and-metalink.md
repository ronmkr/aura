# ADR 0013: Cloud Storage and Metalink Integration

## Status
Accepted

## Context
`Aura` aims to extend `aria2`'s protocol support to modern cloud storage services (S3, Google Drive) while maintaining full support for legacy `aria2` specialties like Metalink.

## Decision
1. **Cloud Adapter**: We will implement a specialized worker type that uses existing Rust SDKs (e.g., `aws-sdk-s3`) but wraps them in the **Protocol Worker** actor interface. This allows the core engine to treat a cloud object like a standard seekable byte-stream.
2. **Metalink Integration**: The **Orchestrator** will use a **Metalink Resolver** to break down a `.metalink` file into its component URIs. Each URI is assigned to a **Protocol Worker**, and the **Piece Selector** uses the provided checksums for verification.
3. **Unified Progress**: Regardless of the source (Cloud, HTTP, BitTorrent), all progress is unified in the **Bitfield**, enabling cross-source "Racing" (e.g., racing a slow S3 download against a fast HTTP mirror).

## Alternatives Considered
- **Direct Cloud SDK usage**: Calling cloud APIs directly from the Orchestrator. *Rejected:* Violates encapsulation and makes the engine dependent on specific cloud libraries.

## Consequences
- **Pros**: Extensible protocol support, unified view of disparate data sources, and robust fallback mechanisms.
- **Cons**: Cloud SDKs can be heavy; we should make them optional feature flags in `Cargo.toml`.
