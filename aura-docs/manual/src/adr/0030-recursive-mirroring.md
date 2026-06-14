# ADR 0030: Recursive Mirroring and HTML Parsing

## Status
Implemented (2026-05-27, PR #142)

## Context
`wget` is the industry standard for recursive site mirroring. Standard multi-protocol downloaders do not support this natively. To be a true multi-functional crawler, `Aura` must be able to crawl web pages and discover linked resources.

## Decision
1. **Recursive Crawler**: We will implement a `RecursiveCrawler` component that uses a high-performance HTML parser (e.g., `tl` or `lol-html`) to extract URIs from `<a>`, `<img>`, `<link>`, and `<script>` tags.
2. **Link Normalization**: The crawler will resolve relative URIs against the base URL and filter them based on user-defined "Stay on Host" or "Stay in Directory" policies.
3. **Queue Integration**: Discovered URIs will be enqueued as new **Download Tasks** with a "Parent GID" link for tracking.
4. **Depth Control**: Users can specify the maximum recursion depth (parity with `wget -l`).
5. **Globbing Integration**: The crawler's base URL / start URL can be defined using brace and bracket glob patterns (conforming to [ADR-0015](0015-url-globbing.md)). The crawler resolves these patterns into multiple seed URLs during initialization to scan and discover assets across all expanded targets.

## Implementation Status (Audit 2026-06-13)
- **Recursive Crawler**: Fully implemented with link extraction and depth controls via PR #142 (2026-05-29).
- **Globbing Support**: Integrated globbing expansions into recursive crawler seeds (2026-06-13).

## Alternatives Considered
- **External Scripting**: Relying on users to pipe output from a crawler into the engine. *Rejected:* Inefficient and doesn't allow the engine to apply its advanced features (like parallel segments) to the discovered links automatically.

## Consequences
- **Pros**: Full parity with `wget` mirroring, enabling `Aura` to replace `wget` for backup and scraping tasks.
- **Cons**: Parsing HTML adds significant CPU overhead; the crawler must be implemented carefully to avoid memory bloat during deep crawls.
