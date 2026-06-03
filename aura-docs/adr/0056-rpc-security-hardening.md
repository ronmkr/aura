# ADR 0056: Daemon RPC Security Hardening

## Status
Implemented (2026-06-03, PR #215)

## Context
The Aura daemon exposes a JSON-RPC 2.0 / WebSocket server to allow control by remote clients (like the CLI or TUI). The audit identified severe security exposure (GAP-07a, GAP-07b, GAP-41):
1. The RPC token uses a hardcoded default public secret `"aura_secret_token"`.
2. The server binds to `0.0.0.0` (all interfaces) by default, exposing it to the network.
3. CORS configuration is set to permissive (`CorsLayer::permissive()`), letting any cross-origin web request execute RPC commands.
4. RPC is sent in cleartext (HTTP/WS only), allowing local eavesdropping of the auth token.

## Decision
1. **Remove Hardcoded Secret Defaults**: Remove the default `--rpc-secret` value. Require the token to be explicitly configured, or automatically generate a random cryptographic token on the first run and store it in a secure user configuration file (e.g., `~/.aura/rpc_secret`).
2. **Local Bind Default**: Change the default listening interface from `0.0.0.0` to `127.0.0.1` (localhost only). Users must explicitly set a `--bind-address` or equivalent config to expose the port.
3. **Restrict CORS Origins**: Restrict CORS to specific localhost domains and explicitly allowed client origins (such as the companion browser extension origin). Reject all other cross-origin preflights.
4. **Transport Layer Security (TLS)**: Support optional transport security on the RPC interface. Add `--tls-cert` and `--tls-key` configurations to bind axum using HTTPS/WSS.

## Edge Cases
1. **Browser Extension Origins**: Chrome and Firefox extensions do not use standard `http/https` origins. They use protocols like `chrome-extension://<extension-id>` or `moz-extension://<uuid>`. The CORS parser must explicitly support wildcards for these protocols or allow configuring a list of approved extension UUIDs.
2. **File Permissions on Generated Secret**: When writing the random secret token to `~/.aura/rpc_secret`, the file must be created with strict permissions (`0600` on Unix/macOS) before writing content. Under Windows, ACLs must be set to restrict access to the current user only.
3. **Port Binding Availability**: If binding to `127.0.0.1:6800` fails because it is already in use, the daemon must fail to start cleanly with a clear error rather than falling back to an unauthenticated interface.
4. **Multi-Homed and Roaming Interfaces**: When a user connects to a public VPN, a local bind (`127.0.0.1`) remains safe, but if the user explicitly exposes the daemon via `0.0.0.0` or a specific LAN IP, switching network interfaces (e.g. Wi-Fi to Ethernet) must not leak credentials to untrusted networks. TLS must be recommended for non-localhost binds.

## Alternatives Considered
- **No RPC Authentication**: Rejected as it allows complete takeover.
- **Always-on TLS**: Reject running without TLS. *Rejected:* Local loopback connections (TUI/CLI on same host) are secure without TLS, and forcing cert generation for local-only use degrades UX.

## Consequences
- **Pros**: Restores zero-trust security posture; prevents LAN/WAN remote command execution; mitigates CSRF vectors via CORS.
- **Cons**: Users must distribute the generated/configured secret token to the CLI or TUI to authenticate remote sessions.
