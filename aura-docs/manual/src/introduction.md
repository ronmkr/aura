# Introduction

**Aura** is a high-performance, asynchronous download engine written in Rust. It is designed to be the spiritual successor to `aria2`, with a focus on modern protocols, extreme concurrency, and proactive privacy.

## Key Philosophies

- **Actor-Based**: Built using the actor model (via Tokio) for high-speed, non-blocking I/O.
- **Protocol Agnostic**: Seamlessly aggregates data from HTTP, FTP, and BitTorrent (v1 & v2) into a single file.
- **Privacy First**: Built-in VPN kill-switches and SOCKS5 proxy support.
- **Resilient**: Automatic retry, self-healing workers, and session persistence.

Aura can be used as a standalone CLI utility, an interactive TUI dashboard, or a headless daemon for server-side orchestration.
