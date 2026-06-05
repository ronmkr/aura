# ADR 0065: Interactive TUI Architecture & Selective Downloading

## Status
Proposed (2026-06-05)

## Context
The current Aura Terminal User Interface (TUI) in `aura-tui` is a flat, single-loop application that displays a static table of active downloads. While functional for basic monitoring, it lacks the depth required for advanced task management, specifically failing to provide granular insight into individual tasks (like piece availability or throughput history) and lacking the capability to perform interactive operations such as selective file downloading from complex BitTorrent swarms or Metalink packages.

Users expect a "Mission Control" experience, akin to modern graphical tools or advanced TUIs like "Mole", where they can drill down into tasks, visualize performance over time, and selectively toggle files to download within a larger package to save disk space and bandwidth.

To execute this effectively, the design must prioritize **Simplicity and Speed**—world-class engineering principles that ensure the codebase remains maintainable, performant, and approachable for new contributors.

## Decision
We will modernize the `aura-tui` and update the `aura-core` engine to support a highly interactive, stateful architecture, strictly adhering to lightweight, fast design patterns.

1. **Stateful Multi-View TUI Architecture**:
   - Refactor `aura-tui` to use a `ViewRouter` pattern, maintaining a stack of navigable screens (e.g., Dashboard -> Mission Control -> File Selector).
   - **Simplicity Mandate**: The router will be a simple Enum state machine. We will explicitly avoid heavy MVC/MVVM frameworks to keep the TUI rendering loop fast and zero-allocation where possible.
   - Implement a 60/40 split-layout on the main dashboard for simultaneous task listing and real-time detail viewing.
   - Introduce rich data visualization using `ratatui` primitives, notably `Sparkline` for throughput history and `Gauge` for progress.

2. **Interactive File Selection & Shared Piece Logic**:
   - Introduce an interactive Tree-View in the TUI to browse hierarchical file structures inside torrents.
   - Expand JSON-RPC API in `aura-daemon` with `aura.getFiles` and `aura.setFileSelection` methods.
   - Update `aura-core`'s `PiecePicker` and `Storage` engines to handle the "Shared Piece" problem: If two files share a BitTorrent piece and only one file is selected, the engine must still download the shared piece but may use sparse allocation for the unselected file's boundary bytes.
   - **Speed Mandate**: Use flat data structures (like `BitVec`) for tracking file selections and piece overlaps rather than complex nested interval trees to ensure O(1) lookups during the hot-path piece picking loop.

3. **Intelligent Bulk Ingestion**:
   - Implement a centralized `ProtocolDetector` to automatically infer task types (HTTP, FTP, BitTorrent, Metalink) from URIs, local paths, or Info-Hashes.
   - Introduce `aura.addFromFolder` (to recursively scan and ingest `.torrent`/`.metalink` files up to a depth limit) and `aura.addFromFile` (to ingest bulk URL lists).
   - Expose these capabilities via both the TUI ("Discovery Modal") and the CLI for headless parity.

## Consequences
- **Pros**:
  - Significantly improves user experience, transforming the tool from a background daemon monitor into a powerful interactive manager.
  - Saves user bandwidth and disk space by enabling selective downloading.
  - Streamlines the addition of multiple tasks through bulk ingestion.
  - Prioritizing simplicity ensures that the new features do not introduce massive technical debt or degrade daemon performance.
- **Cons**:
  - Introduces complexity to the `PiecePicker` and `Storage` engines to correctly map and skip unselected pieces while handling piece overlaps.
  - Requires handling large, virtualized UI trees to prevent lag when viewing torrents with tens of thousands of files.
  - The TUI binary will become slightly heavier due to the state machine and caching requirements (e.g., throughput history buffers).

## Implementation Status (Audit 2026-06-05)
- **Multi-View Architecture**: Pending (Epic created).
- **Interactive File Selection**: Pending (Epic created).
- **Bulk Ingestion**: Pending (Epic created).
