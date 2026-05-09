# Glossary

A collection of technical terms used throughout the Aura documentation.

### Actor Model
A model of concurrent computation where "actors" are the universal primitives. Actors communicate via messages and have private state. In Aura, the **Orchestrator** and **Storage Engine** are actors.

### ADR (Architecture Decision Record)
A document that captures an important architectural decision made along with its context and consequences.

### Bitfield
A compact data structure (a sequence of bits) representing which pieces of a file have been successfully downloaded.

### BDD (Behavior-Driven Development)
A development process where requirements are written as human-readable "features" and "scenarios" (using Gherkin syntax) which are then automatically tested.

### EWMA (Exponential Weighted Moving Average)
A statistical measure used by Aura to track the throughput of connections. It gives more weight to recent data points, allowing Aura to react quickly to network changes.

### GID (Global Identifier)
A unique string or number representing a specific download task in Aura.

### Global Potential
The sum of the estimated capacities of all known sources for a specific file. Aura scales connections until actual throughput matches the global potential.

### InfoHash
A unique fingerprint for a BitTorrent swarm. BitTorrent v1 uses 20-byte SHA-1 hashes; v2 uses 32-byte SHA-256 hashes.

### Merkle Tree
A tree structure where every leaf node is the hash of a data block, and every non-leaf node is the hash of its children. BitTorrent v2 uses Merkle trees for efficient per-file verification.

### Metalink
An XML-based file format that describes a file and its mirrors (HTTP, FTP, P2P). Aura uses Metalinks to automatically orchestrate multi-source downloads.

### Piece Picker
The logic responsible for deciding which piece of a file to request next. Aura uses a "Rarest-First" strategy for BitTorrent and sequential picking for HTTP.

### Protocol Worker
A lightweight actor in Aura that handles a specific network protocol (e.g., the `HttpWorker`).

### Sequential Aggregator
The component in the **Storage Engine** that reorders out-of-order blocks in memory to ensure they are written to disk in a single sequential sweep.

### Sourced Model
The design where a single download task (MetaTask) can have multiple sources (Subtasks) across different protocols.
