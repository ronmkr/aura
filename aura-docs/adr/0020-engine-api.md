# ADR 0020: Engine API and Library Embeddability

## Status
Accepted

## Context
`aria2` is often used as a standalone binary, but `libaria2` allowed it to be embedded into other C++ applications. For `Aura`, we want to make embeddability a first-class citizen using Rust's async features.

## Decision
1. **Engine API**: We will expose a public Rust API that provides direct control over the **Orchestrator**. 
2. **Handle-based Interaction**: The API will return `TaskHandle` objects that allow users to poll progress, pause/resume, or subscribe to **Telemetry Events** using standard Rust streams/channels.
3. **Async-First**: The entire API will be built on `async/await` and designed to run within any `Tokio` runtime.
4. **Feature Flags**: Protocol-specific dependencies (e.g., `openssl`, `libssh2`) will be optional feature flags to allow users to build a "lean" version of the library.

## Alternatives Considered
- **FFI-Only**: Providing only a C-compatible interface. *Rejected:* Inconvenient for the primary Rust-based target audience.
- **Standalone-Only**: Providing only a CLI binary. *Rejected:* Limits the usefulness of the project for the broader Rust ecosystem.

## Consequences
- **Pros**: Enables `Aura` to be used in GUIs, specialized scrapers, and high-performance backend services.
- **Cons**: Requires maintaining a stable public API surface alongside the CLI.
