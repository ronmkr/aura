# ADR 0047: Automated Release Pipeline

## Status
Implemented

## Context
Currently, our CI/CD process only runs tests and formatting checks. Users who wish to run `Aura` must build it from source using `cargo`, which is a significant barrier to adoption. To provide a seamless experience, we must automate the generation and distribution of pre-compiled binaries across major platforms (Windows, macOS, Linux).

## Decision
We will implement an automated release pipeline utilizing GitHub Actions:

1. **Trigger**: The pipeline will trigger automatically upon pushing a new tag (e.g., `v1.2.0`).
2. **Cross-Compilation**: We will use matrix builds in GitHub Actions to compile native binaries for:
   - `x86_64-unknown-linux-gnu` (Linux)
   - `x86_64-apple-darwin` & `aarch64-apple-darwin` (macOS)
   - `x86_64-pc-windows-msvc` (Windows)
3. **Packaging**: The compiled binaries will be compressed (ZIP for Windows, TAR.GZ for Linux/macOS) alongside essential documentation (`README.md`, `LICENSE`).
4. **GitHub Releases**: The artifacts will be published automatically as a formal GitHub Release under the respective version tag, making them instantly downloadable for end users.

## Consequences
- **Pros**: Drastically lowers the barrier to entry for users. Ensures a consistent and reproducible build environment.
- **Cons**: Increases the complexity of the CI/CD configuration. Requires maintaining cross-compilation toolchains and handling platform-specific quirks during the build process.
