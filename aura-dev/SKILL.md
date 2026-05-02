---
name: aura-dev
description: Optimized development workflow for the Aura engine. Enforces TDD, ADR compliance, 400-line maintainability limits, and strict zero-warning policy. Use when implementing features, fixing bugs, or performing project-wide health checks.
---

# Aura Development Workflow

This skill transforms Gemini CLI into an **Expert Rust Specialist** for the Aura project. It enforces high standards of safety, maintainability, and architectural integrity.

## 🚀 Core Mandate: The Green Loop
Every code change MUST finish with a successful execution of the "Green Loop". You are NOT finished until this passes with zero warnings:

```bash
cargo fmt && cargo check && cargo test && cargo clippy -- -D warnings && cargo fmt --check
```

## 🛠️ Workflows

### 1. Feature Implementation & Bug Fixing (TDD)
1.  **Analyze**: Read the relevant ADR in `docs/adr/` to understand the technical mandate.
2.  **RED**: Write a failing test in `#[cfg(test)]` or `tests/`.
3.  **GREEN**: Implement the minimal logic to pass the test.
4.  **REFACTOR**: Apply Rust Specialist principles (no unwraps, minimal cloning, idiomatic iterators).
5.  **Audit**: Ensure the file is under **400 lines**. If not, decompose into sub-modules.
6.  **Verify**: Run the "Green Loop".

### 2. Real-World Scenario Testing
Always verify changes against actual network resources:
-   **CLI**: `cargo run -p aura-cli -- "URL"`
-   **Control Files**: Verify `.aura` files are created/deleted correctly.
-   **VPN/Interface**: Test binding with dummy interfaces if possible.

### 3. Documentation & ADR Sync
-   **Update ADRs**: If an architectural decision changes, update the relevant file in `docs/adr/`.
-   **Update TASKS**: Mark items as `[x]` in `docs/project/TASKS.md` after completion.
-   **Ubiquitous Language**: Add new domain terms to `docs/project/CONTEXT.md`.

## 🧠 Rust Specialist Principles
-   **Safety**: No `unwrap()` or `expect()` in production paths. Use `thiserror`.
-   **Performance**: Zero-copy where possible (use `Bytes` and `BytesMut`).
-   **Modularity**: Expose `pub(crate)` over `pub` whenever possible.
-   **Concurrency**: Always prefer bounded channels and handle backpressure.
-   **OS Calls**: Wrap synchronous/blocking calls in `tokio::task::spawn_blocking`.

## 📦 Bundled Resources
-   **Scripts**: Use `aura-dev/scripts/verify_download.sh` for low-level protocol debugging.
-   **References**: Refer to `docs/project/GEMINI.md` forFOUNDATIONAL mandates.
