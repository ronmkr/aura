# FAQ

Frequently asked questions about the Aura download engine.

## General

### What makes Aura different from `aria2`?
Aura is written in Rust and uses the **Actor Model** for concurrency. This allows it to handle much higher connection counts with lower memory overhead. It also natively supports **BitTorrent v2**, **VPN Kill-switches**, and **Adaptive Scaling** out of the box.

### Does Aura support browser extensions?
Yes, via the **Browser Bridge**. It exposes a JSON-RPC 2.0 interface that can be used by extensions to send download requests to the Aura daemon.

### Is it safe to use for sensitive data?
Aura is designed with a **Privacy-First** mindset. Features like the VPN Kill-switch and DNS-over-HTTPS (DoH) are built-in specifically to prevent your real IP address and browsing history from leaking to ISPs or trackers.

## Performance

### How many connections can Aura handle?
Aura is limited primarily by your OS's file descriptor limit (`ulimit`). In testing, Aura has successfully managed over 500 concurrent connections across multiple swarms and mirrors without significant CPU impact.

### Why does Aura create large files immediately?
Aura uses **Sparse Files** (via `fallocate` or `set_len`). This reserves the total space required for the download on disk, preventing "Disk Full" errors from occurring mid-download after hours of progress.

## Compatibility

### Can I use Aura on Windows or macOS?
Yes. Aura is written in cross-platform Rust. While some features like `kTLS` are Linux-only, the core engine, CLI, TUI, and BitTorrent logic work seamlessly on Darwin (macOS) and Windows.

### Does Aura support v1-only BitTorrent swarms?
Yes. Aura uses a **Hybrid InfoHash** model. It can connect to both v1 and v2 peers simultaneously for the same file.
