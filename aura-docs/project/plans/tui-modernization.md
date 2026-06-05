# Blueprint: Interactive TUI Modernization (Galactic Dashboard 2.0)

This document serves as the high-level roadmap and technical specification for modernizing the Aura Terminal User Interface (TUI). It is designed to be accessible to any developer, providing context, logic explanations, and architectural diagrams.

---

## 🌌 The Vision
Transform the current static table into a "Mission Control" center inspired by modern TUIs like **Mole**. The new interface will support deep interactivity, real-time visualization, and intelligent task management.

---

## 📚 Glossary of Terms
- **Mission**: A single download task (HTTP, FTP, BitTorrent, or Metalink).
- **GID**: Galactic ID - the unique identifier for every mission.
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

## 🛰️ Phase 1: Foundation & Dashboard 2.0
**Objective**: Build the framework for navigation and the split-screen layout.

### Technical Logic:
- **Split Layout**: Use a 60% wide Task List and a 40% wide Detail Panel.
- **Reactive Selection**: When a user highlights a task on the left, the right panel updates instantly with that task's specific metadata (GID, Path, Speeds).
- **Visuals**: Use Ratatui's `Gauge` for progress bars and `Sparkline` for throughput history.

---

## 🔍 Phase 2: Intelligent Discovery
**Objective**: Let users add tasks by pointing to folders or "Mission Files" (URL lists).

### Technical Logic:
- **Protocol Detector**: A function that "peeks" at a string and decides if it's a URL, a Magnet link, or a Torrent file path.
- **Bulk Ingestion**:
  - `addFromFolder`: Scans for `.torrent` files and automatically launches missions for all found items.
  - `addFromFile`: Reads a `.txt` file where each line is a new download target.

---

## 📂 Phase 3: Selective Downloading (File Selector)
**Objective**: Deciding exactly which files you want from a 10GB torrent package.

### The "Shared Piece" Challenge:
In BitTorrent, data is divided into fixed-size "Pieces" (e.g., 1MB).
- **Edge Case**: If File A ends at byte 1,000,500 and File B starts at 1,000,501, they both share "Piece #10".
- **Logic**: If a user selects File A but NOT File B, Aura **must still download** Piece #10.
- **Implementation**: We will create a `FileMask` that maps selected files to their required byte ranges.

---

## 🛡️ Phase 4: Edge Case & Safety Checklist

### 1. Performance & Scale
- **Large Trees**: If a torrent has 50,000 files, the TUI must not lag. We will use **Virtualized Lists** to only render what's visible on screen.
- **RPC Debouncing**: If a user scrolls fast, we won't spam the daemon. We wait 200ms before asking for task details.

### 2. UI Resilience
- **Terminal Resize**: If the window is too small, we hide the detail panel to keep the main list usable.
- **Input Capture**: While typing a search query, the "Q" key should type the letter 'q' instead of quitting the app.

---

## ✅ Verification Plan
1. **Unit Tests**: Test the `ProtocolDetector` with 50+ malformed URIs.
2. **Integration Tests**: Simulate a "Mission File" with 1,000 URLs to ensure the Daemon handles it without crashing.
3. **Manual Audit**: Test the Tree-View with deep folder structures (5+ levels).
