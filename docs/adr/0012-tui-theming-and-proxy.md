# ADR 0012: Themeable TUI and Proxy Connector

## Status
Accepted

## Context
`Aura` needs to be visually appealing and network-flexible. This requires a way to establish connections through various tunnels and a UI that can be customized to match user preferences.

## Decision
1. **Proxy Connector**: All **Protocol Workers** will use a common `ProxyConnector` trait. This trait handles the initial handshake (SOCKS5, HTTP CONNECT) before returning an `AsyncRead + AsyncWrite` stream. This allows the core protocol logic (HTTP/BitTorrent) to remain proxy-agnostic.
2. **Theme Provider**: The TUI will use a `ThemeProvider` that maps logical UI components (e.g., `ProgressBar`, `TaskItem`, `Header`) to specific Ratatui styles (colors, symbols).
3. **CSS-like Theming**: These styles will be loaded from the **Configuration Manager**, allowing users to define themes in their `.toml` file.

## Alternatives Considered
- **Hardcoded Themes**: Too restrictive for a modern CLI tool.
- **Worker-managed Proxies**: Leads to redundant code in every protocol implementation.

## Consequences
- **Pros**: Clean network abstraction, highly customizable UI, and easier support for future protocols (like I2P or Tor).
- **Cons**: Adds a small amount of abstraction overhead to every network connection.
