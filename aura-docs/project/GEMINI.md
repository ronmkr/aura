# Aura: Project Mandates & Instructions

This document defines the foundational mandates and workspace instructions for Gemini when working on the `Aura` project. These instructions take absolute precedence over all other defaults.

## Tech Stack
- **Language & Runtime**: Rust (2021 edition), Tokio (Asynchronous runtime)
- **UI & Presentation**: Ratatui (Terminal User Interface, "The Pilot")
- **Headless & Daemon**: Axum / JSON-RPC 2.0 / WebSockets ("The Ghost")
- **Storage Engine**: Sled (embedded database), custom piece-buffer journal, zero-copy buffer pool
- **Network & Protocols**: Reqwest (HTTP), Suppaftp (FTP), custom BitTorrent client, Crab-NAT / igd-next (NAT traversal)

## Build & Run Commands
- **Fast Build**: `cargo build --workspace`
- **Release Build**: `make release`
- **Quality Verification ("Green Loop")**: `make green-loop` (formats, runs clippy, tests, modularity-check, and benches)
- **Format Code**: `make fmt` (runs `cargo fmt --all`)
- **Clippy Check**: `make clippy` (runs `cargo clippy --workspace -- -D warnings`)
- **Clean Workspace**: `make clean`
- **Build Manual**: `make docs` (compiles mdBook manual)

### Persona Execution
- **Run CLI ("Sprinter")**: `make run-cli ARGS="<URI>"`
- **Run Daemon ("Ghost")**: `make run-daemon`
- **Run TUI ("Pilot")**: `make run-tui`

## Testing Strategy
- **Run All Tests**: `make test` (or `cargo test --workspace`)
- **Integration Tests**: Place in `tests/` directory (e.g., Cucumber tests: `make test-cucumber`)
- **Mocking**: Use `mockall` to mock trait dependencies (especially network or filesystem interactions)
- **Parameterized Tests**: Use `rstest` for clean cases and test fixtures
- **Property-Based Tests**: Use `proptest` for complex boundary calculations, glob parsing, or segmenter logic
- **Benchmarks**: Benchmark hot-paths under `benches/` using `criterion`; verify compile via `make bench`
- **Coverage**: Run with `cargo llvm-cov` targeting 80%+ line coverage

## Engineering & Coding Standards
- **Rust Excellence**: Adhere strictly to the "Expert Rust Specialist" principles: no panics, structured error handling with `thiserror`, type safety via newtypes, and idiomatic async patterns.
- **Strict File Length Limits**: 
  - Optimal file size: 50–150 lines.
  - Soft ceiling: 300 lines (evaluate for splitting).
  - Hard cap: **400 lines maximum per source file** (must decompose into submodules if exceeded).
- **Separation of Tests**: 
  - **No inline `mod tests {` blocks** inside production files.
  - Place unit tests in a separate test file (e.g., `tests.rs` or `*_tests.rs`).
  - Reference it from the production file using:
    ```rust
    #[cfg(test)]
    #[path = "tests.rs"]
    mod tests;
    ```
- **Error Handling**: 
  - Never `unwrap()` or `expect()` in production.
  - Use `thiserror` for custom error types in libraries/crates.
  - Use `anyhow` with `.context()` in application paths (CLI, TUI, main bin) and tests.
- **Data vs Logic**: Separate structural models (e.g., `types.rs`, `models.rs`) from execution logic.
- **The Facade Pattern**: Expose clean public APIs at module boundaries (`mod.rs`, `lib.rs`) and hide deep internals in private submodules.
- **Concurrency**: Prefer bounded channels (`tokio::sync::mpsc::channel`), enforce backpressure, and wrap blocking sync/OS calls in `tokio::task::spawn_blocking`.

## Security & Safety
- **Credential Protection**: The `Secret Scrubber` must be used for all telemetry and logging. Never log full URIs if they contain user:pass.
- **Sandboxing**: All file operations must be validated against the `Sandbox Root`.
- **VPN Safety**: The `Traffic Kill-switch` mandate (ADR 0035/0038) must be enforced for any task bound to a specific interface. Refuse traffic if the authorized tunnel is unavailable.

## Workflow Mandates
- **Zero Warnings**: Treat all compiler and Clippy warnings as errors. Run `cargo clippy -- -D warnings` before every commit.
- **Requirement Audit & Edge Cases**: Before beginning any implementation, you MUST audit the relevant ADRs in `aura-docs/adr/` and check corresponding GitHub Issues to identify constraints and potential edge cases (e.g., OS-specific limits). Do not write code without formulating real-world scenarios first.
- **TDD Workflow**: Always follow the Red-Green-Refactor cycle. A task is not started until a failing test (RED) exists, and not finished until the test passes (GREEN) and the code is idiomatic (REFACTOR).
- **Issue Tracking**: All technical debt, bugs, and feature requests are tracked exclusively on GitHub. Never create local issue markdown files.
- **Branch Protection**: All changes must be submitted via **Pull Requests** to the `main` branch. Direct pushes to `main` are prohibited.
- **Validation**: Every implementation task is incomplete without comprehensive unit tests and behavioral verification.
- **Documentation**: Update `CONTEXT.md` for new domain terms and maintain the ADR sequence in `aura-docs/adr/`. All project documentation must reside in `aura-docs/project/`.
- **No Emojis**: Keep a professional, emoji-free tone in all docs and commits.

## Project Structure
- [aura/](aura/) — Persona Switcher and bootstrap entry point (`main.rs`)
- [aura-cli/](aura-cli/) — CLI downloader ("Sprinter")
- [aura-daemon/](aura-daemon/) — Headless service & JSON-RPC/WebSockets server ("Ghost")
- [aura-tui/](aura-tui/) — Terminal dashboard cockpit ("Pilot")
- [aura-core/](aura-core/) — Core logic actors:
  - `orchestrator/` — Orchestrates download tasks, mapping, and VPN interfaces
  - `storage/` — Disk I/O scheduling, sequential writing, and piece-buffer journaling
  - `worker/` — Protocol workers (HTTP, FTP, BitTorrent, etc.)
  - `piece_picker/` — Piece selection and Work Stealing/Racing strategies
  - `throttler/` — Global token bucket rate limiting
  - `vpn/` — Native VPN interface controllers
- [aura-docs/](aura-docs/) — Architectural Decision Records (ADRs) and User Manual source
