# Aura: Project Mandates

This document defines the foundational mandates for Gemini CLI when working on the `Aura` project. These instructions take absolute precedence over all other defaults.

## ⚙️ Engineering Standards
- **Rust Excellence**: Adhere strictly to the "Expert Rust Specialist" principles: no panics, structured error handling with `thiserror`, type safety via newtypes, and idiomatic async patterns.
- **Actor Integrity**: Maintain strict decoupling between the Orchestrator, Storage Engine, and Protocol Workers. All communication must happen via type-safe channels.
- **Performance First**: Prioritize zero-copy paths and asynchronous I/O. Every significant I/O or architectural change must be accompanied by an ADR.

## 🛡️ Security & Safety
- **Credential Protection**: The `Secret Scrubber` must be used for all telemetry and logging. Never log full URIs if they contain user:pass.
- **Sandboxing**: All file operations must be validated against the `Sandbox Root`.
- **VPN Safety**: The `Traffic Kill-switch` mandate (ADR 0035) must be enforced for any task bound to a specific interface.

## 🏗️ Workflow Mandates
- **TDD Workflow**: Always follow the Red-Green-Refactor cycle. A task is not started until a failing test (RED) exists, and not finished until the test passes (GREEN) and the code is idiomatic (REFACTOR).
- **Validation**: Every implementation task (Milestone) is incomplete without comprehensive unit tests in the core and behavioral verification via the CLI persona.
- **Git Protocol**: Never stage or commit changes unless explicitly instructed with the exact phrase: "Commit these changes".
- **Documentation**: Update `CONTEXT.md` immediately when new domain terms are introduced and maintain the ADR sequence in `docs/adr/`.
