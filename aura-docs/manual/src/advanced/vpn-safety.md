# VPN Safety & Privacy

Privacy is a core pillar of Aura. Unlike traditional downloaders that rely on the OS to route traffic (which can fail silently), Aura proactively monitors the network environment to prevent sensitive data leaks.

## The Active Kill-switch (ADR 0038)

Aura implements an **Active Network Interface Lock**. When `force_tunnel = true` is enabled in `Aura.toml`, Aura enforces a strict hardware-level isolation.

### Implementation Mechanism:
1.  **Interface Heartbeat**: The Orchestrator spawns a background monitor that pings the OS kernel for active network interfaces every `vpn.check_interval_secs` (default: 5s).
2.  **State Verification**: Aura verifies that the authorized interface (e.g., `tun0` for WireGuard, `utun3` for OpenVPN) is in the `UP` and `RUNNING` state.
3.  **Atomic Suspension**: If the interface vanishes (e.g., VPN tunnel crashes), the Orchestrator instantly dispatches a `KillSwitch` broadcast to all actors.
4.  **Socket Termination**: All Protocol Workers immediately drop their TCP/UDP sockets and flush their memory buffers. No further data is transmitted, even if the OS tries to route it over the default (unprotected) gateway.

## SOCKS5 Proxy Support

For users who prefer proxying over a full-system VPN:
- **Unified Proxy Connector**: Supports HTTP, HTTPS, and SOCKS5 (with authentication).
- **BitTorrent Anonymity**: Both the Tracker announcements and the peer-to-peer data transfers are routed through the proxy.
- **Credential Integration**: Proxy usernames and passwords can be safely stored in the `[credentials]` section or a `.netrc` file.

## DNS over HTTPS (DoH) & TLS (DoT)

To prevent ISP-level tracking of your download sources via DNS logs:
- **Encrypted Resolution**: Aura uses the `hickory-resolver` to query upstream resolvers (Cloudflare/Google) over port 443 (DoH) or 853 (DoT).
- **Bootstrap IPs**: Bypasses local system resolvers entirely by using hardcoded bootstrap IPs to find the encrypted resolvers, preventing "DNS hijacking."

## Interface Roaming Reconnector (ADR 0034)

Aura handles network transitions (e.g., switching from Wi-Fi to Ethernet) gracefully:
- **Event Monitoring**: Uses `rtnetlink` (on Linux) to listen for routing table changes.
- **Auto-Pause**: Instantly pauses workers when the default gateway changes to prevent partial data writes during the transition.
- **Auto-Resume**: Re-binds sockets and resumes downloads as soon as a stable internet connection is restored.

## Captive Portal Protection

Hostile environments (like public Wi-Fi) often redirect download requests to HTML login pages. 
- **MIME Inspection**: Aura inspects the first few bytes of every HTTP response.
- **Redirection Logic**: If Aura expects a binary payload (e.g., a `.zip` or `.iso`) but receives an `html/text` body from a redirected URL, it automatically pauses the task with a `CaptivePortal` warning to prevent your file from being corrupted with login page data.

## Kernel TLS (kTLS)

For zero-copy, line-rate performance on supported systems (Linux/FreeBSD), Aura supports **Kernel TLS (kTLS)**:
- **Offloading Encryption/Decryption**: Symmetric decryption/encryption of HTTPS streams is handled directly in the kernel space.
- **Zero-Copy Piping**: Bypasses user-space memory copies entirely by piping data directly from the network card to the storage engine (e.g., using `sendfile` or direct page splicing), eliminating CPU and memory bandwidth bottlenecks.
