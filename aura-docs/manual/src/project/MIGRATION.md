# Aura: Migration Guide

This document outlines breaking changes and migration paths when moving to Aura, or when upgrading across major Aura versions.

## Migrating From Legacy Tools To Aura

Aura is a next-generation download engine. While we aim for operational familiarity with legacy utilities, the underlying architecture is fundamentally different.

### 1. Configuration (cli Args Vs. `aura.toml`)

Legacy download tools rely heavily on extensive CLI flags (e.g., `-x 16 -s 16`). While Aura supports basic CLI arguments for quick usage, the primary configuration mechanism is the `Aura.toml` file.

**Migration:**
- Move global configurations (like global limits, user agents, proxy settings) into `Aura.toml`.
- See `Aura.example.toml` for the full schema.

### 2. Task State & Control Files (`.part` And `.aura`)

Legacy download tools use custom track files to track piece completion.
Aura uses `.aura` files, which are binary-serialized structures that track piece bitfields, dynamic mirror states, and active chunk allocations.

**Migration:**
- **Incompatible:** Legacy tracking files cannot be resumed directly by Aura. You must start the download fresh, though you can use the target file if you implement a manual integrity scan (future feature).

### 3. Rpc Interface (xml-rpc Vs. Json-rpc Over Websockets)

Legacy tools expose XML-RPC and JSON-RPC over HTTP and WebSockets.
Aura exclusively uses JSON-RPC over WebSockets for bi-directional event streaming (Decision 0016).

**Migration:**
- Update your integration scripts to connect via WebSockets (`ws://localhost:6800`).
- Aura supports legacy JSON-RPC payloads for backward compatibility, but native methods (e.g., `aura.subscribe`) provide richer event streams.

---

## Aura V0.x -> V1.0 Breaking Changes

### API Refactoring (taskhandles)

In early pre-v1.0 builds, `Engine::add_task` returned an `mpsc::Receiver`. This has been refactored into the `TaskHandle` struct to provide object-oriented control.

**Before:**

```rust
let mut rx = engine.add_task(url).await;
rx.recv().await;
```

**After (Migration):**

```rust
let handle = engine.add_task(url).await?;
let mut stream = handle.events();
stream.next().await;

// You can now also pause/resume directly
handle.pause()?;
```

### Module Decompositions

The core networking modules have been split. If you are importing internal logic:
- `aura_core::torrent::logic` is now spread across `aura_core::bt_worker`, `aura_core::bt_task`, and `aura_core::bitfield`. Update your `use` statements accordingly.
