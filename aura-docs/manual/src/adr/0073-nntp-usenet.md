# Decision 0073: NNTP Usenet Protocol Worker

## Status

Implemented (2026-06-11, PR #294)

## Context

Usenet (NNTP) remains a highly popular protocol for high-speed, high-bandwidth bulk data retrieval. Content on Usenet is typically split into small articles, posted to specific newsgroups, and encoded using yEnc to handle binary data over text-based NNTP connections. Metadata for these downloads is bundled in XML-based `.nzb` files. To support Usenet as a first-class download protocol in Aura's unified sourced-aggregation model, we need a native NNTP worker.

## Decision

1. **Dedicated NNTP Worker**: We will implement a dedicated `NntpWorker` that implements the standard `ProtocolWorker` trait, allowing it to register with the `Orchestrator` and participate in multi-source downloads.
2. **NZB Parsing & Job Setup**: The engine will parse `.nzb` files to extract segment metadata, article message IDs, and target filenames, feeding them into the piece planning pipeline.
3. **On-the-fly yEnc Decoding**: To avoid heavy disk writing and CPU overhead of double buffer copying, yEnc decoding will be performed directly on incoming socket buffers before passing the clean binary bytes to the `StorageEngine`.
4. **Connection Pooling & SSL/TLS**: Implement support for standard NNTP commands (`GROUP`, `BODY`, `QUIT`) over both unencrypted TCP and secure TLS ports (Nntps).

## Alternatives Considered

- **External Downloader Delegation**: Delegate NZB downloads to local instances of `SABnzbd` or `NZBGet`. *Rejected:* Violates Aura's core design goal of being a single self-contained, high-performance binary with unified speed throttling and scheduling.

## Consequences

- **Pros**: Direct Usenet downloading within Aura, support for mixed-source racing (e.g. Usenet + HTTP mirror), unified bandwidth control, and zero external dependencies.
- **Cons**: Increased parsing complexity (yEnc, NZB XML) and potential memory pressure if buffer allocation is not carefully governed.

## Implementation

- **Worker Module**: Implemented in `aura-core/src/worker/nntp/worker.rs` with native yEnc parser and command pipeline.
