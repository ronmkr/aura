# Decision 0038: Native VPN Integration (OpenVPN, WireGuard)

## Status

Implemented (2026-06-02, PR #194)

## Context

Privacy is a core mandate for `Aura`. While we have implemented an "Active Kill-switch" (Decision 0035/0034) that halts traffic if an interface drops, users currently have to manage the VPN connection itself (OpenVPN, WireGuard) outside of Aura. For a "set-and-forget" experience, Aura should be able to monitor, verify, and potentially trigger VPN connections.

## Decision

1. **VPN Provider Abstraction**: Implement a `VpnProvider` trait to support different VPN backends.
2. **External Controller Mode**: Instead of bundling the full VPN client (which would significantly increase binary size and complexity), Aura will support "Controller Mode" for:
  *  **WireGuard**: Interact via `wg` CLI or `ipc`.
  *  **OpenVPN**: Interact via the Management Interface (TCP/Telnet).
3. **Mandatory Tunnel Enforcement**: If a VPN profile is configured, the `Orchestrator` will refuse to start any `ProtocolWorker` until the `VpnProvider` confirms the tunnel is secure.
4.  **Auto-Reconnect**: Aura will attempt to trigger the VPN client's reconnect mechanism if the kill-switch is triggered.

## Implementation Status (audit 2026-05-09)

- **Active Kill-switch**: **Implemented** in `aura-core/src/orchestrator/mod.rs`. It monitors interface status via the `VpnProvider` trait and sends a `KillSwitch` event to all subtasks if the interface drops.
- **VPN Provider Abstraction**: **Implemented** in `aura-core/src/vpn/mod.rs`.
- **Interface Monitor**: **Implemented** in `aura-core/src/vpn/mod.rs`.
- **Native Controllers (WireGuard/OpenVPN)**: **Implemented** in `aura-core/src/vpn/` (2026-06-02, PR #194).

## Consequences

- **Pros**: Unmatched privacy automation; Aura becomes the first major download engine with native WireGuard/OpenVPN awareness.
- **Cons**: Requires additional OS-level permissions (to query `wg` or `openvpn` status); increased configuration surface.
