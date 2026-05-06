# Aura: Project Mandates

This document defines the foundational mandates for Gemini CLI when working on the `Aura` project. These instructions take absolute precedence over all other defaults.

## ⚙️ Engineering Standards
- **Rust Excellence**: Adhere strictly to the "Expert Rust Specialist" principles: no panics, structured error handling with `thiserror`, type safety via newtypes, and idiomatic async patterns.
- **Maintainability**: Enforce a strict limit of **400 lines per source file**. Any logic exceeding this must be decomposed into logical sub-modules.
- **Actor Integrity**: Maintain strict decoupling between the Orchestrator, Storage Engine, and Protocol Workers. All communication must happen via type-safe channels.
- **Performance First**: Prioritize zero-copy paths and asynchronous I/O. Every significant I/O or architectural change must be accompanied by an ADR.

## 🛡️ Security & Safety
- **Credential Protection**: The `Secret Scrubber` must be used for all telemetry and logging. Never log full URIs if they contain user:pass.
- **Sandboxing**: All file operations must be validated against the `Sandbox Root`.
- **VPN Safety**: The `Traffic Kill-switch` mandate (ADR 0035/0038) must be enforced for any task bound to a specific interface. Refuse traffic if the authorized tunnel is unavailable.

## 🏗️ Workflow Mandates
- **Zero Warnings**: Treat all compiler and Clippy warnings as errors. Run `cargo clippy -- -D warnings` before every commit.
- **TDD Workflow**: Always follow the Red-Green-Refactor cycle. A task is not started until a failing test (RED) exists, and not finished until the test passes (GREEN) and the code is idiomatic (REFACTOR).
- **Branch Protection**: All changes must be submitted via **Pull Requests** to the `main` branch. Direct pushes to `main` are prohibited.
- **Validation**: Every implementation task is incomplete without comprehensive unit tests and behavioral verification.
- **Documentation**: Update `CONTEXT.md` for new domain terms and maintain the ADR sequence in `docs/adr/`. All project documentation must reside in `docs/project/`.
