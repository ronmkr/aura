# Aura 🌌

[![CI](https://github.com/ronmkr/aura/actions/workflows/ci.yml/badge.svg)](https://github.com/ronmkr/aura/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Aura** is a high-performance, asynchronous download engine written in Rust. It is the spiritual successor to `aria2`, reimagined for the modern era with an actor-based architecture, protocol-agnostic orchestration, and memory-safe concurrency.

## 🚀 Features

- **Actor-based Orchestration**: Built on Tokio for massive concurrency and clean decoupling between protocols and storage.
- **Multi-source Aggregation**: Download a single file from multiple sources (HTTP, BitTorrent, FTP) simultaneously with adaptive racing and work stealing.
- **BitTorrent Excellence**: Full support for Trackers (UDP/HTTP), DHT (Kademlia), Seeding, Pipelined Requests, and SHA-1 Hash Verification.
- **FTP Support**: Support for FTP(S) with range-based segment fetching and authentication.
- **Persistent Progress**: State is saved to `.aura` control files, allowing for seamless resumption after restarts.
- **Advanced Networking**: Built-in NAT Traversal (UPnP, NAT-PMP/PCP) and VPN Kill-switch protection.
- **Powerful CLI**: Supports URL globbing (ranges and sets) for easy batch processing.
- **Multiple Personas**: Includes a high-speed CLI, a themeable Ratatui TUI, and a headless daemon controllable via JSON-RPC 2.0.

## 🛠️ Getting Started

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
./target/release/aura-cli https://example.com/file.zip
```

Download a BitTorrent magnet link or torrent file:
```bash
./target/release/aura-cli "magnet:?xt=urn:btih:..."
```

Batch download using globbing:
```bash
./target/release/aura-cli "https://example.com/images/img_[001-100].jpg"
```

## 🏗️ Architecture

Aura is built on a foundation of independent actors:
1. **Orchestrator**: The "brain" that manages task lifecycles, assigns work to workers, and handles global throttling.
2. **Storage Engine**: Manages high-speed asynchronous disk I/O, write aggregation, and atomic file completion.
3. **Protocol Workers**: Lightweight, specialized actors for HTTP, BitTorrent, and FTP that handle protocol-specific logic and data retrieval.

See [design.md](./design.md) for a deep dive into the system design and [CONTEXT.md](./CONTEXT.md) for our ubiquitous language.

## 🤝 Contributing
Please read [CONTRIBUTING.md](./CONTRIBUTING.md) for our engineering standards and TDD workflow.

## 📜 License
This project is licensed under the MIT License - see the LICENSE file for details.
