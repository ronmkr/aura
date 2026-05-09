# Contributing to Aura

Thank you for your interest in contributing to Aura! We welcome contributions from everyone.

## 🧠 Engineering Standards

We maintain a high bar for code quality. All contributions must adhere to:

1.  **Expert Rust Specialist Principles**: Safety first, idiomatic expressiveness, and zero-cost abstractions.
2.  **Mandatory TDD Workflow**:
    *   **Red**: Every PR must include a failing test that demonstrates the new feature or bug.
    *   **Green**: Implementation code must make the test pass.
    *   **Refactor**: Code must be cleaned and optimized before submission.
3.  **No Panics**: Use `Result` and `Error` types. Never use `unwrap()` or `expect()` in library code.
4.  **Zero Warnings**: All compiler and Clippy warnings must be resolved.

## 🏗️ Pull Request Protocol

1.  **Fork the Repository**: Create your own fork and work on a feature branch.
2.  **Architecture (ADRs)**: Significant architectural changes require a new Architecture Decision Record in `aura-docs/adr/`.
3.  **Glossary**: Update `aura-docs/project/CONTEXT.md` if you introduce new domain concepts.
4.  **Verification**: Before submitting, run:
    ```bash
    cargo test
    cargo clippy -- -D warnings
    cargo fmt --all -- --check
    ```
5.  **Small PRs**: Prefer small, focused PRs over massive ones.

## 🛡️ Security

If you find a security vulnerability, please do **not** open a public issue. Refer to our [Security Policy](SECURITY.md) for reporting instructions.

## 🤝 Code of Conduct

Please be respectful and professional in all interactions. See [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for details.
