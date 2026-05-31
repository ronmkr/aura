# VPN Safety & Privacy

Privacy is a core pillar of Aura. Unlike traditional downloaders that rely on the OS to route traffic, Aura proactively monitors the network environment to prevent data leaks.

## The Active Kill-switch

Aura implements a **Network Interface Lock** (ADR 0035/0038). When enabled, Aura will *only* send traffic over a specific authorized interface (e.g., `tun0` for VPNs).

### How it works:
1. **Interface Monitoring**: The Orchestrator spawns a background monitor that pings the OS for active interfaces every 2 seconds.
2. **Immediate Halting**: If the authorized interface disappears or becomes unreachable, the Orchestrator sends a `KillSwitch` event.
3. **Atomic Suspension**: All Protocol Workers immediately drop their sockets and clear their memory buffers. No data is sent over the default (unprotected) gateway.

## SOCKS5 Proxy Support

For users without a full-system VPN, Aura supports global and per-task **SOCKS5 Proxies**.
- **Peer Traffic**: All BitTorrent peer connections are routed through the proxy.
- **Tracker Anonymity**: Tracker announcements (HTTP/UDP) are also proxied to prevent IP exposure to trackers.

## Native VPN Integration (Experimental)

Aura can act as a **VPN Controller** (ADR 0038). It can monitor the health of WireGuard or OpenVPN tunnels and attempt to trigger a reconnect if the connection drops, providing a "set and forget" experience.

## DNS over HTTPS (DoH)

To prevent ISP-level tracking of your download sources, Aura uses **DNS over HTTPS**.
- **Hickory DNS**: Uses the `hickory-resolver` for non-blocking, encrypted DNS resolution.
- **Bootstrapping**: Aura bypasses local system resolvers and queries Cloudflare or Google DNS directly over port 443.

## Interface Roaming Reconnector

Aura includes a robust **Interface Roaming Reconnector** (ADR 0034) that monitors system routing tables and netlink events.
- **Auto-Pause**: If your network interface drops or changes (e.g., disconnecting from office Wi-Fi), Aura instantly pauses all protocol worker actors to avoid leaking data or wasting bandwidth.
- **Auto-Resume**: When a new default route or interface is established (e.g., switching to mobile data or home Wi-Fi), Aura automatically rebinds the sockets to the new route and resumes all operations seamlessly.

## Captive Portal Protection

Hostile network environments (like hotel or Starbucks Wi-Fi) often redirect download requests to landing pages. Aura's **Captive Portal Detector** prevents download file corruption:
- **Redirection Interception**: Aura intercepts initial HTTP/HTTPS redirect chains and inspects HTML bodies when expecting binary payloads (e.g., `.iso`, `.zip`).
- **Graceful Pausing**: If a captive login/redirect page is detected, the task is safely paused with a descriptive state warning rather than writing the HTML payload to disk and corrupting your download.

## Kernel TLS (kTLS)

For zero-copy, line-rate performance on supported systems (Linux/FreeBSD), Aura supports **Kernel TLS (kTLS)**:
- **Offloading Encryption/Decryption**: Symmetric decryption/encryption of HTTPS streams is handled directly in the kernel space.
- **Zero-Copy Piping**: Bypasses user-space memory copies entirely by piping data directly from the network card to the storage engine (e.g., using `sendfile` or direct page splicing), eliminating CPU and memory bandwidth bottlenecks.
