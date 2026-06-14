# Aura

[![CI](https://github.com/ronmkr/aura/actions/workflows/ci.yml/badge.svg)](https://github.com/ronmkr/aura/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Aura** is a high-performance, asynchronous download engine written in Rust. It is the spiritual successor to `aria2`, reimagined for the modern era with an actor-based architecture, protocol-agnostic orchestration, and memory-safe concurrency.

## Features

- **Actor-based Orchestration**: Built on Tokio for massive concurrency and clean decoupling between protocols and storage.
- **Multi-source Aggregation**: Download a single file from multiple sources (HTTP, BitTorrent, FTP) simultaneously with **Adaptive Racing** and **Work Stealing** (ADR 0005).
- **BitTorrent Excellence**: Full support for BitTorrent v1 & v2 (BEP 52), Trackers (UDP/HTTP), DHT (Kademlia), Seeding, Pipelined Requests, and SHA-256 Merkle verification.
- **FTP Support**: Support for FTP & FTPS (TLS) with exponential retry logic, range-based segment fetching, and authentication.
- **Persistent Progress**: State is saved to `.aura` control files, allowing for seamless resumption after restarts.
- **Advanced Networking**: Built-in NAT Traversal (UPnP, NAT-PMP/PCP), DNS-over-HTTPS (DoH/DoT) resolution, and VPN Kill-switch protection.
- **Powerful CLI**: Supports URL globbing (ranges and sets) for easy batch processing.
- **Multiple Personas**: Includes a high-speed CLI, a themeable Ratatui TUI, and a headless daemon controllable via JSON-RPC 2.0.

## Usage

Aura features a unified binary (`aura`) that acts as a standalone CLI, a headless daemon, or a TUI dashboard.

### Docker (Recommended for Servers)

You can easily run the Aura daemon inside a Docker container:

```bash
# Run the background daemon on port 6800
docker run -d \
  --name aura-daemon \
  -p 6800:6800 \
  -v $(pwd)/downloads:/downloads \
  ronmkr/aura daemon --rpc-port 6800
```

Or use the CLI directly via Docker to download a file:

```bash
docker run --rm \
  -v $(pwd)/downloads:/downloads \
  ronmkr/aura "https://example.com/file.zip"
```

### Native Binary

If the `aura` binary is in your `PATH` (or using the compiled binary `./target/release/aura` directly):

```bash
# Run the interactive TUI dashboard
./target/release/aura tui

# Run the headless background daemon
./target/release/aura daemon

# Download a file using the CLI
./target/release/aura "https://example.com/file.zip"
```

## Documentation

- **[Aura User Manual](https://ronmkr.github.io/aura/)**: The comprehensive online guide to using Aura, covering CLI, TUI, and advanced features.
- **[Rust API Docs](https://ronmkr.github.io/aura/api/aura_core/)**: Technical documentation for developers embedding the engine.
- **[Architecture Deep Dive](https://ronmkr.github.io/aura/advanced/architecture.html)**: Detailed mapping of our actor model and data flows.
- **[ADR Index](https://ronmkr.github.io/aura/advanced/adr-index.html)**: The "why" behind our technical decisions.

## Getting Started

### Installation
Ensure you have Rust and Cargo installed, then clone the repository:

```bash
git clone https://github.com/ronmkr/aura.git
cd aura
cargo build --release
```

### Basic Usage
Download a file via HTTP:
```bash
./target/release/aura https://example.com/file.zip
```

Download a BitTorrent magnet link or torrent file:
```bash
./target/release/aura "magnet:?xt=urn:btih:..."
```

Batch download using globbing:
```bash
./target/release/aura "https://example.com/images/img_[001-100].jpg"
```

## Architecture

Aura is built on a foundation of independent actors:
1. **Orchestrator**: The "brain" that manages task lifecycles, assigns work to workers, and handles global throttling.
2. **Storage Engine**: Manages high-speed asynchronous disk I/O, write aggregation, and atomic file completion.
3. **Protocol Workers**: Lightweight, specialized actors for HTTP, BitTorrent, and FTP that handle protocol-specific logic and data retrieval.

See [ARCHITECTURE.md](aura-docs/manual/src/advanced/architecture.md) for a deep dive into the system design and [CONTEXT.md](aura-docs/manual/src/project/CONTEXT.md) for our ubiquitous language.

## Configuration

Aura uses an optional TOML configuration file (e.g., `Aura.toml`) to tune performance:

* **Storage Engine Tuning**:
  * `read_ahead_kb`: Configures the seeding read-ahead buffer capacity for BitTorrent upload paths (defaults to `128KB`).
  * `write_buffer_kb`: Configures the sequential flush aggregator threshold for out-of-order write buffering (defaults to `4MB`).
* **Adaptive Scaling**:
  * `min_connections_per_task` / `max_connections_per_task`: Controls the bounds for the adaptive concurrency scaler in the Orchestrator.

See [Aura.example.toml](Aura.example.toml) for a fully annotated list of all options and their defaults.

## Contributing
Please read [CONTRIBUTING.md](CONTRIBUTING.md) for our engineering standards and TDD workflow.

## License
This project is licensed under the MIT License - see the LICENSE file for details.

