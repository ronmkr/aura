# ADR 0053: BEP 12 Multitracker Compliance

## Status
Implemented (2026-06-02, PR #197)

## Context
The current tracker client in Aura queries all trackers simultaneously. This behavior is non-compliant with BEP 12 (Multitracker Metadata Extension) and can cause unnecessary network storming/load on public trackers. We need to respect the creator's priority tiers while maintaining robust peer discovery.

## Decision
1. **Tier Sequential Processing**: Process tracker tiers sequentially. If at least one tracker in a tier responds successfully, the client stops and does not query subsequent tiers.
2. **Parallel Within Tiers**: Contact all trackers in the active tier in parallel to maximize performance and responsiveness.
3. **Randomization**: Shuffle URLs in each tier when the torrent is first loaded.
4. **Promotion**: Move successful tracker URLs to the front of their tier in the internal cache for future announcement loops.
5. **Deduplication**: Filter out duplicate tracker URLs across tiers when first parsing the torrent's announce list.

## Consequences
- **Pros**: Compliant with BEP 12 specification; prevents overwhelming trackers; respects backup tiers.
- **Cons**: Slightly slower first-run discovery time if early tiers are unresponsive, though offset by caching successful trackers.
