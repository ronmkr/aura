# ADR 0030: Recursive Mirroring and HTML Parsing

## Status
Accepted

## Context
`wget` is the industry standard for recursive site mirroring. `aria2` does not support this natively. To be a true successor to both `aria2` and `wget`, `Aura` must be able to crawl web pages and discover linked resources.

## Decision
1. **Recursive Crawler**: We will implement a `RecursiveCrawler` component that uses a high-performance HTML parser (e.g., `tl` or `lol-html`) to extract URIs from `<a>`, `<img>`, `<link>`, and `<script>` tags.
2. **Link Normalization**: The crawler will resolve relative URIs against the base URL and filter them based on user-defined "Stay on Host" or "Stay in Directory" policies.
3. **Queue Integration**: Discovered URIs will be enqueued as new **Download Tasks** with a "Parent GID" link for tracking.
4. **Depth Control**: Users can specify the maximum recursion depth (parity with `wget -l`).

## Alternatives Considered
- **External Scripting**: Relying on users to pipe `wget` into `aria2`. *Rejected:* Inefficient and doesn't allow the engine to apply its advanced features (like parallel segments) to the discovered links automatically.

## Consequences
- **Pros**: Full parity with `wget` mirroring, enabling `Aura` to replace `wget` for backup and scraping tasks.
- **Cons**: Parsing HTML adds significant CPU overhead; the crawler must be implemented carefully to avoid memory bloat during deep crawls.
