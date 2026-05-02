# Context: Aura

The Rust-based successor to aria2, focusing on high-performance, asynchronous download orchestration using an actor-based architecture.

## Glossary

### User & Orchestration
#### Download Task
The atomic unit of user-facing control (pause, resume, delete). It represents a single logical download operation. A task is **Phase-aware**, transitioning from **Metadata Exchange** to the active downloading phase once it has "matured" and the full file structure is known.

#### Orchestrator
The central coordinator (also referred to as the **Manager**) responsible for the lifecycle of **Download Tasks**, handling external RPC requests, and mediating between different **Protocol Workers**. It serves as the **Source of Truth** for the entire system.

#### Persona Switcher
A bootstrap component that determines the engine's operation mode (**CLI**, **TUI**, or **Daemon**) based on command-line flags.

#### RPC Server
A multi-protocol gateway (JSON-RPC, WebSocket) that enables **Remote Attachment** for the TUI and integration with third-party Web UIs.

#### Themeable TUI
An interactive terminal interface built on **Ratatui** that supports customizable visual themes.

#### Browser Bridge
A specialized component that intercepts download requests from web browsers and funnels them into the **Orchestrator**.

#### Engine API
The public Rust-native interface for `Aura`, optimized for direct integration into other Rust applications.

#### Hook Manager
A lifecycle orchestration service that executes user-defined actions triggered by task state changes.

#### Lifecycle Controller
A high-level service that monitors the engine and executes automated system-level actions (e.g., shutdown) once all tasks are complete.

#### Conflict Handler
The component responsible for resolving file name collisions using policies like **Overwrite**, **Auto-rename**, or **Skip**.

#### Chain Orchestrator
A high-level logic that allows one **Download Task** to spawn another (e.g., HTTP -> BitTorrent).

#### Temporal Scheduler
A service that allows users to define time-based policies for **Download Tasks**.

#### Recursive Crawler
A specialized logic (parity with `wget -m`) that parses HTML/CSS files to discover and enqueue further links.

#### Stream Output Handler
A component that allows downloaded data to be piped directly to `stdout`.

#### Tenant Context
An isolation layer that associates **Download Tasks** with specific users, enforcing per-user permissions and quotas.

#### Shutdown Coordinator
A system-level service that manages the graceful exit of the engine, ensuring all data is flushed and states are saved.

#### Landing Page Resolver
An advanced logic layer that identifies when a URI points to a web page containing a download link rather than the file itself. It uses the **Recursive Crawler**'s logic to find the "Direct Link" automatically.

### Core Engine & Strategy
#### Piece Selector
A strategy-based component within the **Orchestrator** that determines the next optimal **Piece** to be fetched. It supports **Bi-directional Signaling** for work assignment and cancellation.

#### Work Stealer
Logic within the **Piece Selector** that identifies slow network streams using a **Performance Delta** and employs a **Racing** strategy.

#### Segmenter
The component responsible for partitioning continuous byte-streams (HTTP/FTP). It uses the **Boundary Aligner** to ensure its virtual **Pieces** are compatible with multi-source downloads.

#### Boundary Aligner
A coordination logic within the **Sourced Aggregator**. It ensures that segments fetched from non-pieced protocols (like HTTP) align exactly with the piece boundaries defined in the master metadata.

#### Policy Manager
A decision-making component that governs the lifecycle of tasks, handling **Self-healing** and retries.

#### Seeding Policy Manager
A decision-making component that governs the duration and limits of the **Seeding** phase.

#### Error Classification
The categorization of failures into **Worker Error**, **Task Error**, and **Engine Error** scopes.

#### Global Token Bucket Throttler
A centralized rate-limiting component that independently controls global, per-tenant, and per-task speeds.

#### URL Globber
A pre-processor that expands pattern-based URIs (e.g., `[1-100]`) into a batch of individual tasks.

#### Integrity Scrubber
A maintenance service that performs background validation of local data to identify and fix corrupt pieces.

#### Generation Tracker
A versioning system that assigns monotonic **Generation IDs** to piece assignments to prevent "Zombie Writes" during racing.

#### Sequential Aggregator
A component within the **Storage Engine** that reorders write requests to facilitate large, sequential disk writes.

#### Rot-Detection Daemon
A background service that periodically re-verifies completed downloads to detect "Bit Rot" over time.

### Security, Safety & Privacy
#### Sandbox Root
A mandatory security boundary within the **Resource Mapper**. All paths are resolved relative to this root, and traversal attempts are strictly neutralized.

#### Path Normalizer
A utility that ensures filenames are compatible with the target filesystem (Unicode normalization, case-folding, and reserved character removal).

#### Traffic Kill-switch
A privacy feature within the **Interface Binder** that halts all network operations if a bound VPN interface becomes unavailable.

#### Secret Scrubber
A security utility that automatically redacts sensitive information (passwords, API keys) from logs and telemetry events.

#### Backpressure Controller
A safety mechanism that manages actor mailbox sizes and pauses data ingestion when the storage engine is congested to prevent OOM.

#### Resource Governor
A system-level service that enforces hard limits on metadata size, recursion depth, and concurrent connections. It includes a **Dangerous Port Filter**.

#### Dangerous Port Filter
A security mechanism within the **Resource Governor** that blocks outbound connections to sensitive system ports (e.g., SMTP/25, SSH/22).

#### Hash Throttler
A security component that rate-limits CPU-intensive integrity checks to prevent "Hashing DoS" attacks.

#### Punycode Normalizer
A utility within the **Privacy-Enhanced Resolver** that converts IDNs to ASCII and flags visually deceptive domains using a **Homograph Detector**.

#### Captive Portal Detector
A networking safeguard that validates incoming HTTP responses against expected MIME types and file sizes. It prevents the engine from silently saving HTML login pages when operating on restricted public networks.

#### MIME Validator
A safety safeguard that inspects the `Content-Type` header of an incoming stream. It ensures that the received data matches the expected file type (e.g., rejecting `text/html` when the user expects an `application/octet-stream`).

### Networking & Protocols
#### Protocol Worker
An asynchronous actor specialized in a specific protocol (HTTP, BitTorrent, FTP, NNTP). It operates on an **Orchestrated Pull** model with **Abort Signaling** and manages **Redirect Chains**.

#### Redirect Manager
A coordination component within the **Protocol Worker** (specifically HTTP) that manages 3xx status codes. It ensures that redirect chains are followed safely, prevents infinite loops, and maintains security boundaries.

#### Proxy Connector
An abstraction layer for establishing network connections through intermediaries (HTTP, SOCKS5, VPN).

#### Privacy-Enhanced Resolver
A networking service supporting **Async DNS**, **DNS over HTTPS (DoH)**, and local caching.

#### Happy Eyeballs
A dual-stack connection algorithm (RFC 8305) used to minimize latency by attempting IPv4 and IPv6 connections simultaneously.

#### Credential Provider
A service that resolves authentication data from `netrc`, cookies, and secure keychains.

#### Peer Discovery Actor
A background service (DHT, PEX, Trackers, LPD) responsible for finding new peers.

#### Peer Registry
A per-task component that maintains known peers and tracks **Peer Reputation**.

#### Traffic Obfuscator
A privacy component within the BitTorrent **Protocol Worker** that implements Message Stream Encryption (MSE/PE). It masks P2P traffic from Deep Packet Inspection (DPI) by ISPs.

### Storage & Data
#### Storage Engine
A centralized service that manages all disk I/O, supporting **Buffered I/O** (via the aggregator) and **Mapped I/O**. It includes the **Piece-Buffer Journal**.

#### Piece-Buffer Journal
A persistent temporary store in the **Storage Engine** that saves partial, unverified pieces across restarts to prevent progress regression.

#### Journaled State Store
A robust implementation of state persistence that uses an atomic "write-then-swap" strategy to prevent metadata corruption.

#### Boundary Splitter
A logic component that handles **Pieces** spanning multiple physical files.

#### Bit-Bucket File
A virtual file entry in the **Resource Mapper** used for **Padding Files** (BEP 47), avoiding physical junk file creation.

#### Pre-allocation
The process of reserving total disk space at task start to prevent fragmentation and mid-download failure.

#### Atomic Completion
The strategy of writing to a `.part` file and only renaming to the target name after 100% verification.

#### Merkle Tree Store
A specialized database for managing the hierarchical hash trees required by BitTorrent v2.

#### Buffer Pool
A centralized memory management system for caching piece data and enabling **Zero-Copy Path** operations.

#### Disk I/O Scheduler
A component that optimizes disk operations using **Async I/O (io-uring)** and **FADV Strategy**.

#### Resource Mapper
A per-task component that maps logical file structures to physical paths, supporting renaming and mirroring.

#### Filesystem Adapter
A component that detects storage capabilities (NFS/SMB, sparse files) and applies **Adaptive I/O**. It uses an **Allocation Prober** for performance diagnostics.

#### Allocation Prober
A diagnostic tool within the **Filesystem Adapter** that performs "test writes" to determine the true performance of the filesystem's allocation method.

#### Mirroring Mode
An operational state for Cloud Adapters that synchronizes local directories with remote sources (parity with `rclone sync`).

#### COW-Aware Allocator
A specialized logic within the **Filesystem Adapter** that detects Copy-On-Write filesystems (e.g., ZFS, Btrfs). It dynamically alters the **Pre-allocation** strategy to prevent fragmentation.

#### Kernel TLS (kTLS)
An advanced optimization utilized by the **Zero-Copy Path**. It offloads HTTPS/TLS decryption to the kernel or network hardware.
