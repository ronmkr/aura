# FAQ: Frequently Asked Questions

Expert answers to common questions about Aura's design, capabilities, and compatibility.

##  Performance & Architecture

### How does Aura achieve "extreme" concurrency?
Aura uses an **Actor Model** powered by the **Tokio** runtime. Unlike thread-per-connection models, actors are lightweight and asynchronous. This allows Aura to manage thousands of active BitTorrent pieces and HTTP ranges with negligible memory overhead and zero blocking of the main event loop.

### What is the "Racing Work Stealer"?
Inspired by multi-core CPU scheduling, Aura implements a **Racing Work Stealer** (Decision 0005). If one mirror (Subtask) is significantly slower than others, the Orchestrator "steals" its assigned ranges and gives them to a faster worker. The first one to finish the range wins; the other is canceled. This bypasses the "99% stall" common in single-source downloaders.

### Does Aura support zero-copy I/O?
Yes. On supported Linux kernels, Aura utilizes **Kernel TLS (kTLS)** to offload encryption and **`sendfile`/`splice`** to pipe data directly between the network card and the disk buffer, bypassing user-space memory copies.

---

##  Privacy & Safety

### Is my IP address exposed during BitTorrent downloads?
By default, yes (as per the BT spec). However, Aura provides built-in **SOCKS5 Proxy** support and a **VPN Kill-switch**. When `force_tunnel` is enabled, Aura will physically block all traffic if your VPN interface (e.g., `tun0`) drops, ensuring your real IP is never leaked to the swarm.

### What does "Captive Portal Protection" do?
Many public Wi-Fi networks (Hotels, Airports) redirect requests to a login page. If Aura starts downloading what it thinks is an `.iso` but receives an HTML login page, it will pause the task instead of writing the HTML to your file, preventing data corruption.

### Is Aura safe against malicious torrents or SSRF attacks?
Yes. Aura includes several enterprise-grade security features:
- **SandboxRoot (Decision 0054)**: Prevents "path traversal" attacks where a malicious file attempts to write outside the download directory (e.g., to your `.ssh/` folder).
- **SSRF Mitigation (Decision 0059)**: Blocks the engine from making requests to private IP ranges, loopback addresses, or dangerous schemes like `file://`, protecting your local network from being scanned or exfiltrated via the RPC API.
- **Log Scrubbing (Decision 0055)**: Automatically redacts passwords and tokens from logs to prevent accidental credential leakage.

---

##  Compatibility

### Can Aura act as a drop-in replacement for standard download clients?
Aura is **protocol-compatible** with standard WebUIs. Its JSON-RPC 2.0 API supports many standard download methods, allowing you to use existing frontends (WebUIs). While CLI flags differ slightly, Aura's goal is to provide a familiar experience with superior defaults.

### Does it support BitTorrent v2?
Yes. Aura is built from the ground up for **BitTorrent v2** (BEP 52). It supports SHA-256 Merkle trees for per-file integrity and supports **Hybrid Torrents** to bridge v1 and v2 swarms.

### Does Aura support Usenet / NNTP?
Not yet. There is an experimental protocol worker stub in the codebase, but active Usenet (NNTP) downloading is currently deferred (tracked as Issue #22).

### Can I choose specific files to download in a torrent?
Yes. Aura supports **Selective Downloading** (Decision 0065). You can use the `aura show-files` and `aura select-files` commands in the CLI, or use the interactive **File Selector** in the TUI (press `f` on a task). Aura correctly handles pieces that are shared between files at boundary points.

### Is there a record of my past downloads?
Yes. Aura maintains an append-only **History Log** (Decision 0062). You can query it via `aura history`. This log persists across daemon restarts and allows you to audit past activity and verify completion of batch jobs.

### Which filesystems are optimized for Aura?
Aura works on all filesystems, but it has specialized optimizations for:
- **EXT4/XFS**: Utilizes `fallocate` for instant, non-fragmented pre-allocation.
- **Btrfs/ZFS**: Detects and applies **No-COW** (Copy-on-Write) attributes to download files to maintain high I/O throughput (Decision 0035).

---

##  Connectivity

### How do I get "Green" status in BitTorrent?
Aura includes **NAT Traversal** actors that attempt to automatically map ports using **UPnP** and **NAT-PMP/PCP**. If your router supports these, you will automatically be reachable from the WAN. If not, manually forward port `6881` (TCP/UDP).

### Does Aura support IPv6?
Yes. Aura implements **Happy Eyeballs** (RFC 8305), attempting both IPv4 and IPv6 connections in parallel and choosing the fastest one to complete the handshake.
