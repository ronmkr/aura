# Aura Architectural Map

This document provides a high-level overview of the Aura architecture using Mermaid diagrams to visualize component interactions and data flow.

## System Overview

Aura is built on a decoupled, actor-based architecture where protocol-specific logic is isolated from the core orchestration and storage engines.

```mermaid
flowchart TD
    subgraph Clients ["User Interfaces"]
        CLI["The Sprinter (CLI)"]
        TUI["The Pilot (TUI)"]
        Web["Web/Remote (e.g. AriaNg)"]
    end

    subgraph Unified_RPC ["Unified RPC & Background Service"]
        Daemon["Aura Daemon (Background)"]
        RPC["Shared JSON-RPC Server"]
    end

    subgraph Core ["Aura Core"]
        direction TB
        Orchestrator["Task Orchestrator"]
        Storage["Storage Engine"]
        
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

    subgraph Hardware ["System Resources"]
        Disk["Disk I/O"]
        Network["Network (TCP/UDP)"]
        Power["Power Management"]
    end

    %% Interactions
    CLI & TUI & Web -- "JSON-RPC / WebSocket" --> RPC
    Daemon -- "Hosts" --> RPC
    CLI -. "Hosts (Standalone)" .-> RPC
    RPC --> Engine["Engine Core"]
    Engine --> Orchestrator
    
    Orchestrator -- "Spawns" --> Workers
    Orchestrator -- "Requests Allocation" --> Storage
    
    Workers -- "Allocate BytesMut" --> Network
    Workers -- "Fetch Data" --> Network
    Workers -- "Write Chunk" --> Storage
    
    Storage -- "Sequential Flush" --> Disk
    
    BT -- "Discovery" --> DHT & Tracker
    BT -- "Manage" --> PeerReg
    BT -- "Strategy" --> Picker
    
    Engine -- "Keep Awake" --> Power
```

## Component Definitions

### 1. User Interfaces (Personas)
- **Aura CLI ("The Sprinter")**: High-speed, one-off download tool. It can operate as a standalone instance (spinning up its own core and RPC server) or act as a client connecting to an existing `aura-daemon`. Features bulk URL ingestion via CLI flags and recursive folder scanning.
- **Aura TUI ("The Pilot")**: Interactive command center built on a stateful Multi-View architecture (Dashboard, Mission Control, File Selector). Features include:
    - **Zero-Friction Ingest**: Seamless OS clipboard polling and terminal drag-and-drop support via bracketed paste.
    - **Command Palette**: Fuzzy-searchable interface (`:` or `Ctrl+P`) and real-time task filtering (`/`).
    - **Interactive File Selection**: Tree-view selection of specific files within BitTorrent and Metalink bundles.
- **Aura Daemon ("The Ghost")**: The background service that persists across sessions, running the core engine and exposing the shared JSON-RPC / WebSocket interface.

### 2. The Engine & Orchestrator
- **Engine**: The top-level coordinator. It manages the global state, configuration, and the lifecycle of the daemon.
- **Protocol Detector**: A centralized gateway that parses incoming URIs, local files, or directories, identifying the correct protocol (HTTP, FTP, BT, Metalink) for bulk and single task ingestion.
- **Orchestrator**: Responsible for a specific task's lifecycle. It spawns protocol workers, manages retries, and coordinates between the swarm and storage.
- **Protocol-Agnostic MetaTask**: The core task metadata struct is decoupled from protocol-specific fields (like BitTorrent seeding ratios). Protocol-specific state is managed via the `TaskExtension` trait and the `extensions` registry.
- **WorkerBuilder Inversion**: Instead of a monolithic builder that knows about all protocols, Aura uses a generic `WorkerOptions` container. Each protocol worker (e.g., `HttpWorker`, `FtpWorker`) independently accepts these options, allowing new protocols to be added without modifying the central builder logic.

### 3. Storage & Memory
- **Storage Engine**: Handles all disk operations. It uses a **Sequential Aggregator** to reorder out-of-order network chunks into contiguous disk writes, minimizing head movement on HDDs and wear on SSDs.
- **Advisory File Locking**: The storage layer employs an `AdvisoryLocker` to acquire `flock` boundaries over active `.part` files, safely managing concurrent write access across multi-process downloads and network shares.
- **Locality-based Memory**: Instead of a centralized pool, Aura leverages the `bytes` crate's atomic reference counting. Workers allocate `BytesMut` buffers which are passed to the storage engine, ensuring **zero-copy** transfer from network to disk.

### 4. Protocol Workers
- **HTTP/FTP**: Handles mirror racing and multi-segmented downloads.
- **BitTorrent**: Manages the complex swarm logic, including:
    - **Piece Picker**: Rare-first and endgame mode strategies.
    - **Peer Registry**: Maintains health and reputation scores for connected peers.
    - **DHT/Tracker**: Handles decentralized and centralized peer discovery.

### 5. System Integration
- **NAT Traversal**: Automatic port mapping via UPnP/NAT-PMP.
- **Power Management**: Prevents the OS from entering sleep mode while active downloads are in progress.
- **VPN Kill-switch**: Ensures traffic only flows through authorized network interfaces.

## Core Data Flow (Download)

```mermaid
sequenceDiagram
    participant U as User
    participant O as Orchestrator
    participant W as Worker
    participant S as Storage
    participant D as Disk

    U->>O: Add Download (URL/Magnet)
    O->>S: Pre-allocate File
    S->>D: Create Sparse File
    O->>W: Spawn Workers
    W->>W: Allocate BytesMut
    W->>W: Fetch Data (Network)
    W->>S: Push Chunk (StorageRequest::Write)
    S->>S: Reorder Chunks (Aggregator)
    S->>D: Flush Contiguous Block
    D-->>O: Progress Update
    O-->>U: UI Update (TUI/CLI)
```

## Task Lifecycle (State Machine)

Aura tasks are phase-aware actors that transition through various maturation levels.

```mermaid
stateDiagram-v2
    [*] --> Initializing
    Initial: Validating URL/Magnet
    
    Initializing --> Metadata_Exchange: Magnet URI
    Initializing --> Active: Direct URL
    
    Metadata_Exchange --> Active: Info-dict Received
    Metadata_Exchange --> Failed: Timeout / Invalid
    
    state Active {
        [*] --> Downloading
        Downloading --> Paused: User Command
        Paused --> Downloading: User Command
        Downloading --> Integrity_Check: 100% Data
        Integrity_Check --> Downloading: Corrupt Pieces
    }
    
    Active --> Seeding: Integrity Verified
    Seeding --> Completed: Ratio Met / User Stop
    Completed --> [*]
    Failed --> [*]
```

## BitTorrent Swarm Discovery

The discovery process is a multi-channel orchestration to maximize peer density.

```mermaid
graph LR
    subgraph Discovery ["Discovery Channels"]
        DHT["DHT (Kademlia)"]
        PEX["PEX (Peer Exchange)"]
        Trackers["HTTP/UDP Trackers"]
        LPD["LPD (Local Peer Discovery)"]
    end

    subgraph Core ["Orchestration"]
        Registry["Peer Registry"]
        Reputation["Reputation Engine"]
    end

    DHT & PEX & Trackers & LPD -- "Peer Addresses" --> Registry
    Registry -- "Filter / Score" --> Reputation
    Reputation -- "High-Quality Peers" --> Worker["BT Protocol Worker"]
    Worker -- "New Peers Found" --> PEX
```

## Storage Engine Internals

The storage engine optimizes for high-throughput sequential I/O to protect hardware health.

```mermaid
graph TD
    subgraph RAM ["Volatile Memory"]
        Agg["Sequential Aggregator (pending_writes)"]
        Dirty["Dirty Buffer (dirty_buffers)"]
    end

    subgraph Disk ["Persistent Storage"]
        Part[".part File"]
        Final["Final File"]
        Sled["Sled DB (v2 Metadata)"]
    end

    Workers["Protocol Workers"] -- "Zero-Copy Write" --> Agg
    Agg -- "Sort & Align" --> Dirty
    Dirty -- "Contiguous Flush" --> Part
    Part -- "Integrity OK" --> Final
    Final -- "Hash Tree" --> Sled
```

## Security & Isolation

Aura enforces strict boundaries between the network and the host system, and isolates tenants from each other when running in shared daemon mode.

```mermaid
flowchart LR
    Network["Untrusted Network"]
    
    subgraph Sandbox ["Aura Security Sandbox"]
        RG["Resource Governor"]
        RBAC["Tenant Context (Auth & Quotas)"]
        Root["Sandbox Root (FS)"]
        Audit["Structured Audit Tracing (JSON)"]
    end
    
    Host["Host OS"]

    Network -- "Traffic Kill-switch" --> RG
    RG -- "MIME/Port Filtering" --> RBAC
    RBAC -- "Path Normalization" --> Root
    Root -- "Authorized I/O Only" --> Host
    RBAC -. "Logs" .-> Audit
```

## Implementation Map

This table maps architectural concepts to their primary implementation files in the Aura workspace.

| Component | Category | File Path |
| :--- | :--- | :--- |
| **Persona Switcher** | Orchestration | `aura/src/main.rs` |
| **The Pilot (TUI)** | Interface | `aura-tui/src/app.rs`, `ui.rs` |
| **Aura Daemon** | Persistent | `aura-daemon/src/lib.rs` |
| **Engine Core** | Orchestration | `aura-core/src/orchestrator/engine.rs` |
| **Task Orchestrator** | Orchestration | `aura-core/src/orchestrator/runner.rs` |
| **Sequential Aggregator** | Storage | `aura-core/src/storage/aggregator.rs` |
| **Advisory Locker** | Storage | `aura-core/src/storage/locker.rs` |
| **Storage Ops** | Storage | `aura-core/src/storage/engine.rs`, `ops.rs` |
| **Worker Builder** | Protocol | `aura-core/src/worker/builder.rs` |
| **Locality Memory** | Memory | `aura-core/src/worker/types.rs` (BytesMut) |
| **HTTP Worker** | Protocol | `aura-core/src/worker/http/mod.rs` |
| **FTP Worker** | Protocol | `aura-core/src/worker/ftp.rs` |
| **BitTorrent Logic** | Protocol | `aura-core/src/worker/bittorrent/worker.rs` |
| **Piece Picker** | Strategy | `aura-core/src/piece_picker/logic.rs` |
| **Peer Registry** | Strategy | `aura-core/src/peer_registry/logic.rs` |
| **DHT Node** | Discovery | `aura-core/src/dht/actor/mod.rs` |
| **Tracker Client** | Discovery | `aura-core/src/tracker/logic.rs` |
| **Power Manager** | System | `aura-core/src/power/logic.rs` |
| **NAT Traversal** | Network | `aura-core/src/nat/logic.rs` |
| **LPD** | Discovery | `aura-core/src/lpd/logic.rs` |

---

> **Note**: This map is current as of Milestone 6. As the project matures, new mappings will be added for Merkle Tree stores and End-game mode logic. See the [ROADMAP.md](ROADMAP.md) for full status.
