---
name: aura-dev
description: Optimized development workflow for the Aura engine. Enforces TDD, ADR compliance, 400-line maintainability limits, and strict zero-warning policy. Use when implementing features, fixing bugs, or performing project-wide health checks.
---

# Aura Development Workflow

This skill transforms Gemini CLI into an **Expert Rust Specialist** for the Aura project. It enforces high standards of safety, maintainability, and architectural integrity.

## 🚀 Core Mandate: The Green Loop
Every code change MUST finish with a successful execution of the "Green Loop". You are NOT finished until this passes with zero warnings:

```bash
make green-loop
```

## 🛠️ Workflows

### 1. Requirement Audit & Edge Case Verification
Before writing a single line of code, you MUST complete this pre-flight check:

**Requirement Audit:**
1.  **Read ADRs**: Consult the relevant files in `aura-docs/adr/`. Identify the core architectural constraints and the "why" behind the design.
2.  **Verify Alignment**: Compare the ADR against the current codebase. If the implementation has diverged or if the requirement is outdated, update the ADR or propose a change before proceeding.
3.  **Constraint Mapping**: For complex requirements, create a "Constraint Map" where every mandatory clause in the ADR (e.g., "Must be atomic", "No data over eth0") is mapped to a specific test case or implementation block.
4.  **Check Previous Implementation**: Search the codebase for similar logic. Identify patterns, shared types, and potential integration points.
5.  **Audit Learnings**: Read `aura-docs/project/LEARNINGS.md` to avoid repeating historical mistakes (e.g., trust-but-verify server headers, redirect loop traps).

**Edge Case Verification:**
Identify and document how the feature handles:
-   **Network Limits**: Latency spikes, packet loss, socket timeouts, and SOCKS5 proxy overhead.
-   **OS Constraints**: File path length limits (Windows `\\?\`), block allocation quirks (`fallocate`), and sandbox violations.
-   **Concurrency Traps**: Race conditions in config reloads (use oneshot sync!), deadlock risks in circular actor dependencies, and channel backpressure.
-   **State Corruption**: Handling malformed persisted state (`.aura` files) and database migration failures.

**Real-World Scenario Definition:**
Formulate concrete, hostile testing scenarios to guide your TDD:
-   *Example (Throttling)*: "A user reloads config to 1KB/s while a 100MB/s stream is active. Does the task instantly react without overflow?"
-   *Example (VPN)*: "The wg0 interface vanishes while 5 protocol workers are mid-read. Are all sockets closed within 500ms?"
-   *Example (I/O)*: "Disk space runs out during a sequential write flush. Is the in-memory buffer preserved or dropped?"

### 2. Feature Implementation & Bug Fixing (TDD)
1.  **Analyze**: Review the requirements and edge cases gathered during the audit phase.
2.  **RED**: Write a failing test in `#[cfg(test)]` or `tests/`.
3.  **GREEN**: Implement the minimal logic to pass the test.
4.  **REFACTOR**: Apply Rust Specialist principles (no unwraps, minimal cloning, idiomatic iterators).
5.  **Maintainability**: Ensure the file is under **400 lines**. If not, decompose into sub-modules.
6.  **Format**: Automatically format all code using `cargo fmt --all` to guarantee style consistency and avoid CI check failures.
7.  **Verify**: Run `make green-loop`.

### 3. Real-World Scenario Testing
Always verify changes against actual network resources:
-   **CLI**: `make run-cli ARGS="URL"`
-   **Daemon**: `make run-daemon`
-   **Dashboard**: `make run-tui`
-   **Control Files**: Verify `.aura` files are created/deleted correctly.
-   **VPN/Interface**: Test binding with dummy interfaces if possible.

### 4. Documentation & ADR Sync
-   **Build Manual**: Run `make docs` to verify the mdBook manual builds correctly.
-   **Update ADRs**: If an architectural decision changes, update the relevant file in `aura-docs/adr/`.
-   **Update TASKS**: Mark items as `[x]` in `aura-docs/project/TASKS.md` after completion.
-   **Ubiquitous Language**: Add new domain terms to `aura-docs/project/CONTEXT.md`.

## 🧠 Rust Specialist Principles
-   **Safety**: No `unwrap()` or `expect()` in production paths. Use `thiserror`.
-   **Performance**: Zero-copy where possible (use `Bytes` and `BytesMut`).
-   **Modularity**: Expose `pub(crate)` over `pub` whenever possible.
-   **Concurrency**: Always prefer bounded channels and handle backpressure.
-   **OS Calls**: Wrap synchronous/blocking calls in `tokio::task::spawn_blocking`.

## 📦 Bundled Resources
-   **Scripts**: Use `aura-dev/scripts/verify_download.sh` for low-level protocol debugging.
-   **References**: Refer to `aura-docs/project/GEMINI.md` forFOUNDATIONAL mandates.
