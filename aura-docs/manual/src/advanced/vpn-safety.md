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
