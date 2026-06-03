# Introduction

**Aura** is a high-performance, asynchronous download engine written in Rust. It is designed to be the next-generation multi-protocol download engine, with a focus on modern protocols, extreme concurrency, and proactive privacy.

## Key Philosophies

- **Actor-Based**: Built using the actor model (via Tokio) for high-speed, non-blocking I/O.
- **Protocol Agnostic**: Seamlessly aggregates data from HTTP, FTP, and BitTorrent (v1 & v2) into a single file.
- **Task Chaining**: Automated multi-step workflows (e.g., HTTP -> BitTorrent handover).
- **Mapping Engine**: Metadata-driven directory organization and conflict management.
- **Multi-Tenancy**: Secure resource isolation and bandwidth quotas for multi-user hosting.
- **Privacy First**: Built-in VPN kill-switches, SOCKS5 proxy support, and DNS-over-HTTPS (DoH/DoT) resolution.
- **Resilient**: Automatic retry, self-healing workers, and session persistence.

Aura can be used as a standalone CLI utility, an interactive TUI dashboard, or a headless daemon for server-side orchestration.
