# ADR 0011: Dynamic Configuration and Hot-reloading

## Status
Accepted

## Context
`aria2` supports hundreds of options, but they are mostly static once the process starts. In `Aura`, we want to support dynamic configuration changes (e.g., changing bandwidth limits or adding a proxy) without restarting the service.

## Decision
1. **Configuration Manager**: We will use a central manager to store all settings.
2. **File Format**: We will prefer **TOML** (e.g., `Aura.toml`) for its balance of readability and Rust ecosystem support.
3. **Hot-reloading**: The manager will use a file-watcher (e.g., `notify` crate) to detect changes to the config file and update the internal state automatically.
4. **Subscriber Pattern**: Components (like the Throttler or TUI) can subscribe to specific configuration changes to react immediately.

## Alternatives Considered
- **Environment Variables**: Too limited for complex hierarchical configuration.
- **Static Config**: Re-reading config only on restart. *Rejected:* Inconvenient for long-running headless servers.

## Consequences
- **Pros**: Better developer and user experience, immediate feedback for configuration changes.
- **Cons**: Requires thread-safe access to configuration (e.g., `Arc<Config>` or `arc_swap`), which adds slight overhead.
