# 50. Integration Tests Suite

Date: 2026-05-27

## Status

Implemented

## Context

As the Aura ecosystem grows (CLI, Daemon, core engine), relying solely on unit tests and manual verification is insufficient. The complex interplay between asynchronous networking, piece-level disk I/O, BitTorrent swarm coordination, and HTTP/FTP worker threads requires a robust end-to-end integration testing framework.

## Decision

We will build a comprehensive, automated Integration Tests Suite utilizing a Behavior-Driven Development (BDD) approach with the `cucumber` Rust crate.
- High-level scenarios will be defined in `.feature` files using Gherkin syntax (e.g., testing storage allocations, swarm magnet link metadata maturation, throttling).
- We will mock specific dependencies where necessary (e.g., simulating HTTP servers with artificial latency/failures, mocking BitTorrent trackers) to ensure deterministic test environments.
- The `make green-loop` standard will formally include `cargo test --workspace` to gate all commits.

## Consequences

- **Pros:** Prevents regressions in complex subsystems (e.g., data corruption bugs, UI/Daemon IPC failures). Provides living documentation of expected system behavior.
- **Cons:** Increases CI time and build complexity. Requires ongoing maintenance of BDD step definitions and test fixtures.
