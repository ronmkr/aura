# ADR 0070: RSS/Atom Feed Subscriptions

## Status
Proposed (2026-06-11 — Issue #290)

## Context
NAS deployments, seedboxes, and home servers often serve as automated media download hubs. Users expect the ability to subscribe to RSS or Atom feeds (e.g., software release feeds, podcasts, media releases) and have matching items downloaded automatically. Aura currently has no feed subscription, polling, or filtering system. However, the `quick-xml` crate is already a project dependency, providing high-performance XML parsing capabilities suitable for RSS/Atom formats.

## Decision
1. Implement an RSS/Atom feed subscription manager in `aura-core/src/rss/`.
2. Define a subscription storage format in `~/.aura/feeds.toml`, containing an array of feeds with fields:
   - `url`: Feed URL.
   - `name`: Human-readable identifier.
   - `poll_interval`: Polling frequency (default: 30 minutes).
   - `filters`: Array of regular expressions or size ranges to match against item titles/attributes.
3. The daemon starts a background actor that regularly polls subscribed feeds:
   - Parse XML using `quick-xml`.
   - Apply filters to the item `<title>`, `<category>`, or size.
   - If a match is found, extract the download URL (from `<enclosure url="..."` or `<link>`) and invoke `Engine::add()`.
4. Deduplicate downloads by tracking ingested GUIDs or URLs in a simple key-value database or text file (`~/.aura/feed_history.db`) to prevent re-downloading historical feed items.
5. Plumb CLI subcommands under `aura feed`:
   - `aura feed add <URL> [--name <name>]`
   - `aura feed remove <URL|name>`
   - `aura feed list`
   - `aura feed refresh`

## Edge Cases
1. **Network Timeout & Failures**: Feeds can become temporarily unavailable or rate-limit the daemon. Poller must implement exponential back-off and respect tracker/feed HTTP status codes.
2. **Duplicate GUID Formats**: RSS items may lack a GUID or use mutable URLs. Fall back to hashing the item title + pubDate to form a unique fingerprint.
3. **Malicious XML**: RSS feeds can carry XML entity expansion attacks (billion laughs). Disable entity expansion in `quick-xml` configuration.

## Alternatives Considered
- **External Automation Tools (Flexget/Autobrr)**: Let users configure external tools to call Aura's JSON-RPC endpoint. *Rejected:* While supported, native RSS parsing is a core requirement for out-of-the-box NAS suitability, and is standard in all competing download managers.
- **TUI-only client RSS reader**: Parse RSS only in the TUI. *Rejected:* Downloads must occur in the background when the TUI is closed.

## Consequences
- **Pros**: Elevates Aura's viability for headless home server automation; utilizes existing XML parser dependency.
- **Cons**: Adds persistent history tracking and background polling scheduling to the daemon core.
