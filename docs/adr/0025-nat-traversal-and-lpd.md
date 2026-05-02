# ADR 0025: NAT Traversal and LAN Discovery

## Status
Accepted

## Context
BitTorrent performance is heavily dependent on "reachability"—the ability for other peers to initiate connections to the client. Manual port forwarding is a significant barrier for users. Additionally, high-speed transfers on local networks are often hampered by unnecessary routing through external gateways.

## Decision
1. **NAT Traversal Actor**: We will implement an actor that uses UPnP and NAT-PMP (via crates like `igupnp` or `nat-pmp`) to dynamically request port mappings from the router at startup.
2. **Local Peer Discovery (LPD)**: We will implement LPD (Multicast DNS/Announce) to discover and connect to peers on the local subnet. LPD peers will be given a "High Priority" status in the **Peer Registry**.
3. **Connectivity Telemetry**: The **Event Bus** will publish events regarding NAT status (Open/Moderate/Closed) to inform the user through the TUI.

## Alternatives Considered
- **Manual Port Forwarding Only**: *Rejected:* Poor UX and limits the user base to technically proficient individuals.
- **Relay-based NAT Traversal (STUN/TURN)**: *Rejected:* Too complex and expensive for a standard download utility.

## Consequences
- **Pros**: Zero-configuration reachability, near-instant LAN speeds, and better integration with modern home networks.
- **Cons**: UPnP is occasionally disabled on routers for security reasons; we must provide clear feedback when it fails.
