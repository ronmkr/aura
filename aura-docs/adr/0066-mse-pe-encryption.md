# ADR 0066: MSE/PE Traffic Encryption

## Status
Implemented (2026-06-13, PR #301)

## Context
BitTorrent protocol traffic is easily identified and throttled by Internet Service Providers (ISPs) using Deep Packet Inspection (DPI). Without protocol encryption, all wire traffic from Aura (including protocol handshakes and block transfers) is in plaintext. Furthermore, many private trackers enforce encryption and refuse connections from clients without Message Stream Encryption (MSE/PE) support. While `CONTEXT.md` references a "Traffic Obfuscator" component implementing MSE/PE, no code currently exists for it in the codebase.

## Decision
1. Implement Message Stream Encryption (MSE/PE) as defined in the BitTorrent protocol specification (standard RC4-based traffic encryption).
2. Add a configuration policy under `[bittorrent]` in `Aura.toml`: `encryption = "prefer" | "require" | "disable"`.
   - `prefer` (default): Attempt encrypted handshake, fall back to plaintext if peer does not support it.
   - `require`: Only establish encrypted connections. Discard plaintext peer connections.
   - `disable`: Only use plaintext connections.
3. The encryption layer will wrap the standard TCP connection using the RC4 algorithm during the handshake phase:
   - Establish Diffie-Hellman key exchange.
   - Synchronize negotiation bytes using RC4 keys derived from the shared secret and InfoHash.
   - Decrypt/encrypt subsequent handshakes and payload packets transparently if negotiated.
4. Integrate this layer in `aura-core/src/worker/bittorrent/protocol/` (specifically under a new `mse.rs` module and integrated into `handshake.rs`).

## Edge Cases
1. **CPU Overhead**: RC4 is computationally cheap but encrypting all stream payloads can add CPU overhead on high-speed connections. The config should support a header-only encryption mode (`crypto_select = 0x01` handshake negotiation) to encrypt only the handshake and leave payload plaintext if maximum throughput is preferred.
2. **Tracker Reporting**: Trackers need to know if a peer is reachable on an encrypted port. Set `support_crypto=1` flag in tracker HTTP announces if encryption is enabled.
3. **InfoHash Obfuscation**: The handshake obfuscates the InfoHash using DH keys. Ensure InfoHash bytes are never leaked during raw negotiation packets.

## Alternatives Considered
- **Plaintext Only**: Maintain existing TCP transport. *Rejected:* Prevents access to encryption-only swarms (common on private trackers) and makes traffic highly susceptible to ISP throttling.
- **WireGuard/VPN Only**: Force users to route all traffic through a VPN. *Rejected:* While Aura supports native VPN integration (ADR-0038), it is heavy, requires external configuration, and does not obfuscate traffic inside the tunnel itself or help with tracker encryption requirements.

## Consequences
- **Pros**: Protects Aura traffic from shallow DPI throttling; grants access to private tracker swarms with strict encryption policies; satisfies the design described in `CONTEXT.md`.
- **Cons**: RC4 encryption increases CPU utilization per peer connection; handshake state machine becomes significantly more complex.
