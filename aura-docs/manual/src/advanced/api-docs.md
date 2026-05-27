# API Documentation (rustdoc)

Aura's codebase is extensively documented using standard Rust doc comments. This documentation provides a deep-dive into the internal types, traits, and functions that power the engine.

## Generating the Documentation

To build the internal API documentation locally, run:

```bash
make docs-api
```

This uses `cargo doc` with the following flags:
- `--workspace`: Generates documentation for all crates in the workspace (`aura-core`, `aura`, etc.).
- `--no-deps`: Skips generating documentation for external dependencies to speed up the process.
- `--document-private-items`: Includes internal (non-public) items to provide full visibility for developers.

## Viewing the Documentation

Once generated, the documentation is available in the `target/doc/` directory. You can open it in your browser:

```bash
open target/doc/aura_core/index.html
```

## Key Modules to Explore

- **`aura_core::orchestrator`**: The central actor and task management logic.
- **`aura_core::storage`**: Asynchronous disk I/O and write aggregation.
- **`aura_core::worker`**: Protocol-specific fetching logic for HTTP, FTP, and BitTorrent.
- **`aura_core::throttler`**: Global and per-task bandwidth rate-limiting.
