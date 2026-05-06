# ADR 0008: Lifecycle-based Task Maturation (Magnet Links)

## Status
Accepted

## Context
When a download is initiated via a Magnet URI, the system does not initially know the file list, total size, or piece count. This prevents the immediate initialization of the **Bitfield** and **Storage Engine**. In `aria2`, this is handled by a special mode, but in `Aura`, we want a unified actor model.

## Decision
We will use a **Phase-based Maturation** model:
1. **Initial State**: A task created via a Magnet URI starts in the **Metadata Exchange** phase.
2. **Metadata Fetch**: The **Orchestrator** instructs **Protocol Workers** to perform a metadata-only exchange (BEP 9 for BitTorrent).
3. **Transition**: Upon receipt of the full metadata, the task is updated with the file info-dict.
4. **Maturation**: The **Orchestrator** triggers "Maturation," which initializes the **Bitfield**, performs **Pre-allocation**, and moves the task into the **Downloading** phase.

## Alternatives Considered
- **Separate Discovery Actor**: Use a temporary actor to find metadata, then create a "real" Task. *Rejected:* Adds complexity in tracking GIDs (Global IDs) and managing user-facing state across two different objects.
- **Lazy Initialization**: Initialize the Bitfield "on-the-fly" as pieces arrive. *Rejected:* Incompatible with BitTorrent where the total piece count must be known to validate any single piece.

## Consequences
- **Pros**: Uniform user-facing API (the task exists from the moment the Magnet link is added), clear lifecycle events for the TUI/RPC.
- **Cons**: Requires the **Orchestrator** to handle dynamic reconfiguration of a running **Download Task**.
