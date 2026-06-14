# Aura: Developer Setup Guide

Welcome to the Aura development team! Aura is a next-generation download engine, built with Rust, Tokio, and an actor-based architecture. This guide will help you set up your local environment, understand our toolchain, and integrate with our Continuous Integration (CI) pipeline.

## 1. Development Environment

Aura is designed to be cross-platform, but active development is best supported on **Linux** and **macOS**. 

### Prerequisites

You will need the following tools installed:
- **Rust Toolchain**: [rustup](https://rustup.rs/) (Stable channel)
- **Make**: Used for our build and test scripts (`make green-loop`)
- **Git**: For version control
- **WireGuard Tools** (`wg`, `wg-quick`): Required for testing the native VPN integration.
  - Linux: `sudo apt install wireguard`
  - macOS: `brew install wireguard-tools`

### Initial Setup

1. **Clone the repository:**
   ```bash
   git clone git@github.com:ronmkr/aura.git
   cd aura
   ```

2. **Verify the installation:**
   ```bash
   cargo build
   cargo test
   ```

## 2. Toolchain and Workflows

We strictly adhere to the "Expert Rust Specialist" principles. We expect zero warnings and high-quality error handling (`thiserror`, no `unwrap`s in production paths).

### The "Green Loop"

Before committing any code, you **must** ensure it passes our automated checks. We wrap this into a single Make target:

```bash
make green-loop
```

This target runs:
1. `cargo fmt --all -- --check` (Style/formatting compliance)
2. `cargo clippy --workspace -- -D warnings` (Strict linting for zero warnings)
3. `cargo test --workspace` (Unit & integration test suites)
4. `cargo bench --workspace --no-run` (Ensures performance benchmarks compile)

### Cargo Extensions (Optional but Recommended)
- **`cargo-watch`**: `cargo install cargo-watch` (Auto-recompile on file changes)
- **`cargo-sweep`**: `cargo install cargo-sweep` (Keep your `target/` directory small)
- **`cargo-nextest`**: `cargo install cargo-nextest` (Faster test execution)

## 3. Continuous Integration (CI)

Aura uses **GitHub Actions** for CI. Our pipeline enforces the same standards as the local `green-loop`.

### Workflows

- **Lint & Format**: Runs `clippy` and `rustfmt` to ensure code quality and style compliance. Fails if there are any warnings.
- **Test Matrix**: Runs `cargo test` across multiple operating systems (Ubuntu, macOS, Windows) to ensure cross-platform compatibility.
- **Security Audit**: Runs `cargo audit` to check for dependencies with known security vulnerabilities.
- **CodeQL**: Automated semantic code analysis to catch memory leaks or logic bugs.
- **Deploy Documentation**: Automatically builds the mdBook manual and rustdoc API documentation upon pushes to `main`, publishing the unified portal directly to GitHub Pages.

### Pull Request Requirements

All development must happen on branches and be merged via Pull Requests. To get a PR approved:
1. All CI checks must pass.
2. The code must not introduce any new `unwrap()` or `panic!()` calls without extensive justification.
3. Relevant tests (unit or integration) must be included.
4. Any architectural changes must be accompanied by an update to the Decisions (`aura-docs/manual/src/adr/`).

Happy Hacking!
