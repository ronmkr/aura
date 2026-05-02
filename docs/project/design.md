# Aura: System Design

This document details the visual and architectural design for the `Aura` personas: CLI, TUI, and Headless/Web.

## 🎭 The Three Personas

### 1. The Sprinter (CLI)
- **Goal**: High-speed, one-off downloads.
- **Visuals**: Minimalist. Single progress bar per file, total speed summary.
- **Workflow**: `Aura-cli <URI>`.

### 2. The Pilot (TUI)
- **Framework**: Ratatui.
- **Layout**:
    - **Header**: Global throughput (Up/Down), Active tasks count, NAT status.
    - **Main Area**: Scrollable list of tasks with interactive selection.
    - **Sidebar/Panel**: Detailed metadata for selected task (Mirrors, Peer map, Bitfield visualization).
    - **Footer**: Keybindings (a: add, p: pause, r: resume, d: delete, q: quit).
- **Theming**: CSS-like theme provider in `Aura.toml`. Supports ANSI 256 colors and Unicode symbols (e.g., Block characters for bitfields).
- **Remote Mode**: Can "Attach" to a remote `Aura-daemon`.

### 3. The Ghost (Headless / Web)
- **Goal**: Persistent service for Docker, NAS, or Seedboxes.
- **RPC Server**: Built on Axum/Tokio.
    - **Protocol**: JSON-RPC 2.0 and WebSockets.
    - **Compatibility**: Standardized to allow existing `aria2` frontends (like **AriaNg**) to connect with minimal adaptation.
- **Docker Design**: Multi-stage build providing a slim alpine/distroless image.

## 🧠 Core Engine Components

### Buffer Pool & Storage
- **Memory**: Centralized `Bytes` pool with pre-allocated chunks.
- **Writing**: **Sequential Aggregator** reorders random pieces into contiguous disk flushes.
- **Safety**: **Atomic Completion** (.part files) ensures no partial files are exposed.

### Connectivity
- **Happy Eyeballs**: Parallel IPv4/IPv6 racing.
- **Privacy DNS**: Built-in DoH support to bypass ISP censorship.
- **NAT Traversal**: Automatic UPnP/NAT-PMP mapping.

## 🏗️ Core Engineering Mandates
- **Actor Integrity**: Strict decoupling via type-safe channels.
- **TDD First**: Every component must be developed using Red-Green-Refactor.
- **Zero-Copy**: Optimize for minimal memory movements.

## 🚀 Implementation Roadmap

### Milestone 1: The Atomic Download (Completed ✅)
- Fundamental Actor skeleton and HTTP single-source download.

### Milestone 2: The "Smart" Buffer (Current)
- Implementation of the `Storage Engine` with real I/O, `.part` files, and the `Buffer Pool`.

### Milestone 3: The Swarm (BitTorrent)
- BitTorrent Protocol Worker, DHT/PEX discovery, and Bitfield management.

### Milestone 4: Hyper-Scale (Aggregator)
- Racing work-stealer, adaptive scaling, and cross-protocol mirror merging.

### Milestone 5: Visuals & Remote
- Ratatui TUI implementation and Axum RPC Server.
