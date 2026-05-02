---
name: Aura-dev
description: Manage the development lifecycle of Aura, including TDD loops, health checks, and binary verification of download links. Use when the user wants to run the app, write/run tests, or verify if a URL is a landing page or a direct binary.
---

# Aura Development Skill

## Quick start

To run a full project health check:
```bash
cargo check && cargo test && cargo clippy -- -D warnings
```

## Workflows

### 1. TDD Loop (Red-Green-Refactor)
Use when implementing a new feature or fixing a bug.
1. **RED**: Add a failing `#[tokio::test]` in the relevant module (core/storage/worker).
2. **VERIFY**: Run `cargo test -p Aura-core --lib module::tests`.
3. **GREEN**: Implement minimal code to pass.
4. **REFACTOR**: Run clippy and cleanup imports.

### 2. Download Verification
Use when a URL fails to download as expected.
1. Run the `verify_download.sh` script with the URI.
2. If it's HTML, analyze headers and body for "Landing Page" markers.
3. Update `HttpWorker`'s `Landing Page Resolver` logic if needed.

### 3. Persona Verification
Use to test the CLI/TUI experience.
1. CLI: `cargo run -p Aura-cli -- <URI> --output <FILE>`
2. Verify file on disk: `ls -l <FILE> && file <FILE>`

## Implementation Rules
- **No Unwraps**: Always use `?` and `thiserror`.
- **Async First**: Use `tokio` for all I/O.
- **Trace Everything**: Use `debug!` and `info!` macros for visibility.
