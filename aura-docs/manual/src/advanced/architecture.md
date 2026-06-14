# Aura Architectural Map
This document provides a high-level overview of the Aura architecture using Mermaid diagrams to visualize component interactions and data flow.
## System Overview
Aura is built on a decoupled, actor-based architecture where protocol-specific logic is isolated from the core orchestration and storage engines. It follows the **Orchestrated Pull Model** (Decision-0001) for work assignment.
```mermaid
flowchart TD
    subgraph Clients ["User Interfaces (Personas)"]
        CLI["The Sprinter (CLI)"]
        TUI["The Pilot (TUI)"]
        Web["Web Dashboard (The Ghost)"]
    end
    subgraph Unified_Binary ["Unified binary (aura)"]
        direction TB
        RPC["Shared JSON-RPC / WebSocket Server"]
        Service["System Service Manager"]
        Watch["Watch Folder (notify)"]
        RSS["RSS Subscription Poller"]
        subgraph Core ["Aura Core Engine"]
            direction TB
            Engine["Engine Hub"]
            Orchestrator["Task Orchestrator"]
            Storage["Storage Engine"]
            Governor["Resource Governor"]
            subgraph Workers ["Protocol Workers"]
                HTTP["HTTP Worker"]
                BT["BitTorrent Worker"]
                FTP["FTP Worker"]
            end
            subgraph BT_Logic ["Swarm Logic"]
                DHT["DHT / PEX"]
                Tracker["Tracker Client"]
                PeerReg["Peer Registry"]
                Picker["Piece Picker"]
            end
        end
    end
    subgraph Hardware ["System Resources"]
        Disk["Disk I/O (Sparse Files)"]
        Network["Network (TCP/UDP)"]
        VPN["VPN Tunnel (WireGuard/OpenVPN)"]
    end
    %% Interactions
    CLI & TUI & Web -- "JSON-RPC (0.0.0.0:6800)" --> RPC
    Service -- "Lifecycle Control" --> Unified_Binary
    Watch -- "Auto-Ingest" --> Engine
    RSS -- "Feed Ingestion" --> Engine
    RPC --> Engine
    Engine --> Orchestrator
    Orchestrator -- "Lifecycle Management" --> Workers
    Orchestrator -- "Pre-allocation" --> Storage
    Orchestrator -- "Backpressure" --> Governor
    Workers -- "Zero-Copy BytesMut" --> Network
    Workers -- "Write-Back Cache" --> Storage
    Storage -- "Sequential Aggregation" --> Disk
    BT -- "Discovery" --> DHT & Tracker
    BT -- "Scoring" --> PeerReg
    BT -- "Strategy" --> Picker
    Engine -- "Monitoring" --> VPN
```
## Component Definitions
### 1. User Interfaces (Personas)
- **Aura CLI ("The Sprinter")**: Optimized for one-off tasks and shell pipelines. It can operate in "Standalone Mode" (booting an ephemeral core) or "Client Mode" (connecting to a local/remote daemon).
- **Aura TUI ("The Pilot")**: An interactive terminal dashboard following the **Stateful View Pattern**. It provides real-time visualization of swarm health, piece distribution, and historical throughput via Braille sparklines.
- **Aura Web ("The Ghost")**: A lightweight, integrated dashboard for headless servers, accessible via standard browsers.
### 2. The Engine & Orchestrator
- **Engine Hub**: The global coordinator for state, configuration, and shared services (Telemetry, Event Bus).
- **Task Orchestrator**: Manages the maturation of tasks (Decision-0008). It spans the lifecycle from URI detection to metadata exchange (BitTorrent info-dicts) and final assembly.
- **Resource Governor**: Implements global memory backpressure and CPU prioritization (Decision-0057), ensuring the daemon remains stable during massive protocol aggregation.
- **Protocol Detector**: A centralized gateway for parsing URIs and expanding local file globs (Decision-0015).

### God Node Decoupling (Decision-0072)
To improve maintainability and testability, Aura migrated from a monolithic "God Node" pattern to a **Trait-Based Decoupling** model:
- **Engine Hub vs. Task Orchestrator**: The `Engine` now serves as a lightweight container for global resources, while the `Orchestrator` handles task-specific state machines.
- **Dependency Inversion**: Components no longer interact with concrete structs. Instead, they depend on specialized traits (e.g., `StorageDispatch`, `TaskController`), allowing for 100% isolated unit testing via mocks (e.g., `MockStorage`).
- **Clean Boundaries**: This decoupling ensures that changes to the BitTorrent protocol worker do not require modifications to the Storage Engine's internal logic.

### 3. Storage & Memory
- **Sequential Aggregator**: Reorders random network chunks into contiguous disk flushes (Decision-0033), protecting hardware longevity.
- **Atomic Completion**: Utilizes `.part` files and transactional renames to ensure no corrupt or partial data is ever exposed as finished (Decision-0003).
- **Zero-Copy Pipeline**: Leverages `BytesMut` reference counting to move data from the network card to the filesystem buffer without intermediate copies.
### 4. Protocol Workers
- **HTTP/FTP**: Implements **Racing Work Stealing** (Decision-0005) across multiple mirrors to maximize bandwidth utilization.
- **BitTorrent**: A fully-featured swarm engine supporting **BitTorrent v2** (Merkle Trees), PEX, DHT, and an advanced **Endgame Mode** (Decision-0039).
### 5. System Integration
- **VPN Kill-switch**: A native monitoring loop for WireGuard and OpenVPN interfaces (Decision-0038), halting all traffic if the secure tunnel drops.
- **Happy Eyeballs**: Dual-stack racing (IPv4/IPv6) for resilient connectivity (Decision-0026).
- **Docker Hardening**: Multi-stage builds and non-root confinement for secure containerized deployment (Decision-0051).
- **Watch Folder Auto-Ingestion**: Filesystem watch loop utilizing `notify` with an active 500ms file-size stabilization loop to debounce writes and ingest torrents/metalinks (Decision-0069).
- **RSS Subscription Poller**: Background feed polling loop inside the daemon to automatically fetch, filter, and ingest matching feed downloads (Decision-0070).
- **System Service Control**: Native system manager service configuration (systemd, launchd, Windows SCM) supporting daemon auto-start on boot (Decision-0071).
## Core Data Flow (The Green Path)
```mermaid
sequenceDiagram
    participant P as Persona/Watch/RSS
    participant E as Engine Hub
    participant O as Orchestrator
    participant W as Worker
    participant S as Storage
    participant D as Disk
    P->>E: Add Task (Magnet/URL/File)
    E->>O: Dispatch Task
    O->>S: Pre-allocate (Sparse)
    O->>W: Spawn Worker(s)
    W->>W: Fetch Segment (Network)
    W->>S: Write (BytesMut)
    S->>S: Aggregate & Sort
    S->>D: Contiguous Flush (.part)
    S-->>O: Segment Verified
    O-->>E: Task Progress
    E-->>P: Status/Event Sync
```
## Implementation Map
This table maps architectural concepts to the verified codebase paths as identified by the Knowledge Graph.
| Component | Path | Primary Role |
| :--- | :--- | :--- |
| **Engine Core** | `aura-core/src/orchestrator/engine.rs` | Global state & event bus |
| **Orchestrator** | `aura-core/src/orchestrator/runner.rs` | Task lifecycle & maturation |
| **Storage Engine** | `aura-core/src/storage/engine.rs` | Disk I/O & Sandbox root |
| **Aggregator** | `aura-core/src/storage/aggregator.rs` | Write-back caching |
| **HTTP Worker** | `aura-core/src/worker/http/mod.rs` | Mirror racing & segmenting |
| **BT Worker** | `aura-core/src/worker/bittorrent/mod.rs` | Swarm management |
| **BT v2 Merkle** | `aura-core/src/torrent/v2/merkle.rs` | Block-level integrity |
| **DHT Manager** | `aura-core/src/dht/mod.rs` | Kademlia routing & protocol facade |
| **NNTP Worker** | `aura-core/src/worker/nntp/worker.rs` | Usenet/Newsgroup segment fetching |
| **VPN Provider** | `aura-core/src/vpn/wireguard.rs` | Tunnel enforcement |
| **RPC Router** | `aura-daemon/src/jsonrpc/router.rs` | Centralized JSON-RPC dispatch |
| **TUI App State** | `aura-tui/src/app/state.rs` | Stateful view management |
| **CLI Client** | `aura/src/cli_client.rs` | Unified binary gateway |
| **RSS Manager** | `aura-core/src/rss/manager.rs` | RSS feed parsing, storage, and matching |
| **Watch Folder** | `aura-core/src/orchestrator/watch.rs` | Filesystem watch loop and debouncer |
| **System Service** | `aura/src/service.rs` | Service control installer and manager |
---
