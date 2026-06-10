# Blueprint: Interactive TUI Modernization (Command Center)

This document serves as the high-level roadmap and technical specification for modernizing the Aura Terminal User Interface (TUI). It is designed to be accessible to any developer, providing context, logic explanations, and architectural diagrams.

---

## 🌌 The Vision
Transform the current static table into a "Mission Control" center inspired by modern TUIs like **Mole**. The new interface will support deep interactivity, real-time visualization, and intelligent task management.

---

## 🚀 Core Principles: Simplicity & Speed (The "Google" Approach)
To ensure this modernization is achievable, maintainable, and lightning-fast, we adhere to world-class engineering standards:
1. **Minimal Abstractions**: The `ViewRouter` will not be an over-engineered framework. It will be a simple, flat Enum state machine (`enum ViewState { Dashboard, Detail(TaskId), ... }`). Keep it simple so any engineer can jump in and contribute immediately.
2. **Zero-Allocation Rendering**: The UI rendering loop must be exceptionally fast. We will borrow references (`&str`, `&[Task]`) during the `ratatui` draw phase rather than cloning data, ensuring zero-allocation screen refreshes.
3. **RPC Efficiency**: Avoid N+1 query problems. The TUI will batch requests to the Daemon and heavily debounce interactions (e.g., waiting 200ms during rapid scrolling before requesting task details).
4. **Straightforward Data Structures**: For selective downloading, avoid complex interval trees unless proven necessary by benchmarks. A simple `BitVec` or flat array map for the `FileMask` will be the starting point. Simplicity minimizes bugs.
5. **Incremental Delivery**: "Done is better than perfect." We will ship the Main Dashboard first, then layer on Discovery, then Selective Downloading. 

---

## 📚 Glossary of Terms
- **Mission**: A single download task (HTTP, FTP, BitTorrent, or Metalink).
- **GID**: Global ID - the unique identifier for every mission.
- **Piece Overlap**: When a single data block (piece) contains bytes from two different files.
- **Sparkline**: A small, high-density line graph (using Braille characters) to show throughput over time.
- **View Router**: The logic that decides which screen to show (Dashboard vs. Mission Control).

---

## 🏗️ Architectural Overview

```text
[ User Input ] <--> [ aura-tui ] <--(JSON-RPC)--> [ aura-daemon ] <--> [ aura-core ]
       ^                  |                             |                    |
   Keys/Mouse       View Management                RPC Handlers        Engine/Workers
```

### 1. The TUI "View" Pattern
Instead of a single large loop, we will use a **Stateful View Pattern**:
- Each screen (Dashboard, Detail, Selector) implements a `View` trait.
- A `ViewRouter` manages the "Screen Stack", allowing users to "drill down" (Enter) and "go back" (Esc).

---

## 🛰️ Phase 1: Foundation & Main Dashboard
**Objective**: Build the framework for navigation and the split-screen layout.

### Technical Logic:
- **Split Layout**: Use a 60% wide Task List and a 40% wide Detail Panel.
- **Reactive Selection**: When a user highlights a task on the left, the right panel updates instantly with that task's specific metadata (GID, Path, Speeds).
- **Visuals**: Use Ratatui's `Gauge` for progress bars and `Sparkline` for throughput history.

---

## 🔍 Phase 2: Intelligent Discovery & Bulk Ingestion
**Objective**: Let users add tasks by pointing to folders or "Mission Files" (URL lists), with full CLI parity.

### Technical Logic:
- **Protocol Detector**: A function that "peeks" at a string and decides if it's a URL, a Magnet link, or a Torrent file path.
- **Bulk Ingestion**:
  - `addFromFolder`: Scans for `.torrent` files and automatically launches missions for all found items.
  - `addFromFile`: Reads a `.txt` file where each line is a new download target.
- **CLI Parity**: 
  - Update `aura-cli` to handle directory paths as arguments (auto-scanning for torrents/metalinks).
  - Update `aura-cli` to accept text files as input (`--from-file` or `aura input.txt`) to enqueue multiple tasks.

---

## 📂 Phase 3: Selective Downloading (File Selector)
**Objective**: Deciding exactly which files you want from a 10GB torrent package, supported in both TUI and CLI.

### The "Shared Piece" Challenge:
In BitTorrent, data is divided into fixed-size "Pieces" (e.g., 1MB).
- **Edge Case**: If File A ends at byte 1,000,500 and File B starts at 1,000,501, they both share "Piece #10".
- **Logic**: If a user selects File A but NOT File B, Aura **must still download** Piece #10.
- **Implementation**: We will create a `FileMask` that maps selected files to their required byte ranges.
- **File-Level Prioritization**: Instead of binary `[x]/[ ]` selection, the tree view should support `[H] High`, `[N] Normal`, and `[S] Skip`. This maps directly to BitTorrent piece-picking algorithms.
- **The Magnet Link "Blind Spot"**: When adding a Magnet link, the file list is unknown until metadata is downloaded. We must introduce a `pause_metadata_complete=true` flag so the engine pauses the task the moment metadata arrives, allowing the user to select files *before* bulk data transfer begins.
- **CLI Parity**:
  - Add `aura show-files <torrent>` to list files with their indices.
  - Add `--select-file 1,3,5-10` flag to `aura-cli` for headless selection.

---

## 🛡️ Phase 4: Edge Case & Safety Checklist

### 1. Performance & Scale
- **Large Trees**: If a torrent has 50,000 files, the TUI must not lag. We will use **Virtualized Lists** to only render what's visible on screen.
- **RPC Debouncing**: If a user scrolls fast, we won't spam the daemon. We wait 200ms before asking for task details.

### 2. UI Resilience & Data Safety
- **Terminal Resize**: If the window is too small, we hide the detail panel to keep the main list usable.
- **Input Capture**: While typing a search query, the "Q" key should type the letter 'q' instead of quitting the app.
- **Post-Download Un-selection (Data Deletion)**: If a user unchecks a file that is already 100% downloaded, the TUI MUST display a highly visible warning (`[!] This will delete X GB of downloaded data`). The Storage engine must then punch a hole (using sparse files) or delete the specific file to reclaim space.

---

## 🌟 Phase 5: Delightful UX & OS Integration ("The Mole Polish")
**Objective**: Elevate the TUI from a "good utility" to a "world-class application" by anticipating user friction and maximizing ergonomics.

- **The "Cmd+K" Command Palette**: Pressing `:` or `Ctrl+P` opens a fuzzy-searchable overlay. Users can type commands (e.g., "pause all") instead of memorizing hotkeys. It also displays the mapped hotkey for discoverability.
- **Advanced Sorting & Multi-Filter (Inspired by `stig`)**: The `/` search should allow complex queries (e.g., `status:active size:>1G`). Allow sorting the Dashboard by clicking column headers or via the Command Palette (e.g., `sort by ETA`).
- **Remote Daemon Management**: Like `transmission-remote-cli`, the TUI should be capable of connecting to remote Aura Daemons (via JSON-RPC + TLS) rather than just the local engine, enabling "Seedbox" management directly from the local terminal.
- **Desktop Notifications**: Integrate native OS notifications (via `notify-rust`). Ping the user when a massive mission completes or if a critical halt (e.g., "Disk Full") occurs, allowing them to confidently "walk away" from the terminal.
- **Drag-and-Drop Terminal Support**: Intercept absolute paths pasted into the terminal (standard behavior when dragging a file into iTerm/Windows Terminal). Instantly recognize `.torrent` or `.metalink` files and trigger the "Add Mission" flow.
- **First-Class Vim Motions**: Full ergonomic support for power users. `j/k` for vertical navigation, `h/l` for tab switching, `gg` for top, `G` for bottom, and `/` for search.
- **Actionable Error Recovery**: Instead of dead-end red error text, the Mission Details panel will suggest fixes based on the error state (e.g., `[Disk Full] -> Press [c] to clear cache`, `[Corrupt] -> Press [s] to run scrubber`).
- **Clipboard Integration (Zero-Click Add)**: The TUI automatically detects URLs/Magnets in the OS clipboard and displays a non-intrusive banner: `🔗 Link detected in clipboard. Press [Enter] to add.`
- **"Open in OS" Support**: When a task is `✅ Complete`, pressing `[o]` utilizes native OS handlers (`open` on macOS, `xdg-open` on Linux, `start` on Windows) to open the downloaded file or reveal it in the file explorer.
- **"Boss Key" / Panic Hide**: Double-tapping `Esc` instantly minimizes the TUI or obfuscates sensitive download names for privacy.
- **Colorblind / High-Contrast Mode**: A `--high-contrast` startup flag that disables green/red color reliance, replacing them with highly distinct ASCII indicators (e.g., `[OK]`, `[ERR]`) for better accessibility.

---

## ✅ Verification Plan
1. **Unit Tests**: Test the `ProtocolDetector` with 50+ malformed URIs.
2. **Integration Tests**: Simulate a "Mission File" with 1,000 URLs to ensure the Daemon handles it without crashing.
3. **Manual Audit**: Test the Tree-View with deep folder structures (5+ levels).
