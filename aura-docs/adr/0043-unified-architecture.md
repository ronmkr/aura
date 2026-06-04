Status: Implemented

# ADR 0043: Unified Architecture and CLI-Daemon-TUI Integration

## Status
Implemented (2026-05-29, PR #144)

## Context
Currently, `aura-cli`, `aura-daemon`, and `aura-tui` operate somewhat independently:
- `aura-cli` instantiates `aura-core` directly and runs the download in the foreground, without exposing the JSON-RPC interface.
- `aura-daemon` instantiates `aura-core` and exposes the JSON-RPC interface, but has no UI.
- `aura-tui` connects to the JSON-RPC interface but cannot interact with downloads started by `aura-cli`.

This leads to a fragmented user experience. A download started via `aura-cli` cannot be monitored by `aura-tui`. The components need to be linked together to provide a unified experience, similar to standard CLI downloaders.

## Decision
We will unify the architecture by ensuring `aura-cli` and `aura-daemon` share the same RPC server capability, and `aura-cli` can act as an RPC client.

1. **RPC in CLI**: We will move the JSON-RPC server logic from `aura-daemon` into a shared library (e.g., `aura-rpc` or `aura-core`), allowing `aura-cli` to optionally spin up the RPC server when running in standalone mode (e.g., `aura-cli --enable-rpc`). This allows `aura-tui` to connect to the CLI process.
2. **CLI as Client**: `aura-cli` will be enhanced to check if an instance of `aura-daemon` is already running on the default RPC port (6800). If so, it will delegate the download request to the daemon via JSON-RPC instead of starting its own `aura-core` instance.
3. **TUI Consistency**: `aura-tui` will remain a pure JSON-RPC client. Thanks to the above changes, it will be able to monitor downloads regardless of whether they were started via the CLI or the Daemon.

## Consequences
- **Pros**: 
  - Unified experience: Users can start a download in the CLI and monitor it in the TUI.
  - Reduced duplication: Shared RPC logic.
  - True CLI integration with background daemon controls.
- **Cons**: 
  - Refactoring required to move the RPC server out of `aura-daemon` into a shared location.
  - `aura-cli` becomes more complex as it needs to handle both "standalone with RPC" and "client to daemon" modes.

## Implementation
- **Unified Architecture**: Implemented in `aura-cli/`, `aura-daemon/`, and `aura-tui/` by sharing JSON-RPC server and client components (2026-05-29, PR #144).
