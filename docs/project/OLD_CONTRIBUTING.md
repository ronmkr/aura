# Contributing to Aura

Thank you for your interest in contributing to the next generation of high-performance downloaders!

## 🧠 Engineering Standards
We maintain a high bar for code quality. All contributions must adhere to:
1. **Expert Rust Specialist Principles**: Safety first, idiomatic expressiveness, and zero-cost abstractions.
2. **Mandatory TDD Workflow**:
    - **Red**: Every PR must include a failing test that demonstrates the new feature or bug.
    - **Green**: Implementation code must make the test pass.
    - **Refactor**: Code must be cleaned and optimized before submission.
3. **No Panics**: Use `Result` and `Error` types. Never use `unwrap()` or `expect()` in library code.

## 🏗️ Pull Request Protocol
- **ADRs**: Significant architectural changes require a new Architecture Decision Record in `docs/adr/`.
- **Glossary**: Update `CONTEXT.md` if you introduce new domain concepts.
- **Verification**: Run `cargo test` and `cargo clippy -- -D warnings` before submitting.
