# ADR 0011: Dynamic Configuration and Hot-reloading

## Status
Implemented (2026-05-06, commit 0777b1ab)

## Context
Traditional download engines support hundreds of options, but they are mostly static once the process starts. In `Aura`, we want to support dynamic configuration changes (e.g., changing bandwidth limits or adding a proxy) without restarting the service.

## Decision
1. **Configuration Manager**: We will use a central manager to store all settings.
2. **File Format**: We will prefer **TOML** (e.g., `Aura.toml`) for its balance of readability and Rust ecosystem support.
3. **Hot-reloading**: The manager will use a file-watcher (e.g., `notify` crate) to detect changes to the config file and update the internal state automatically.
4. **Subscriber Pattern**: Components (like the Throttler or TUI) can subscribe to specific configuration changes to react immediately.
5. **Hierarchical Resolution & Merging**: Config files are resolved in order of precedence: Custom CLI path (`--config`) -> Working Directory (`./Aura.toml`) -> User Home Config (`~/.config/aura/Aura.toml` or `%AppData%\aura\Aura.toml`). Files are recursively merged in order of increasing precedence. CLI flags are then applied as top-level overrides.

## Alternatives Considered
- **Environment Variables**: Too limited for complex hierarchical configuration.
- **Static Config**: Re-reading config only on restart. *Rejected:* Inconvenient for long-running headless servers.

## Consequences
- **Pros**: Better developer and user experience, immediate feedback for configuration changes, zero hardcoded defaults in core logic.
- **Cons**: Requires thread-safe access to configuration (e.g., `Arc<Config>` or `arc_swap`), which adds slight overhead.

## Implementation
- **Primary Files**: `aura-core/src/config/logic.rs`, `aura-core/src/orchestrator/engine.rs`, `aura/src/main.rs`
- **Logic**: Resolves configuration hierarchically, supports merging via TOML table interpolation, and applies CLI overrides before passing down to the Daemon and CLI runtimes. Hot-reloading watches the resolved configuration path.

## Verification
- **Test**: `aura-core/src/config/logic_tests.rs` (Validates TOML parsing, merging hierarchy, and CLI overrides).
- **Cucumber BDD**: `aura-core/tests/features/config.feature` (Validates end-to-end configuration scenarios).
