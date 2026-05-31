# Aura: Public Rust API Documentation

Aura isn't just a daemon; it is designed to be highly embeddable via the `aura-core` crate. This document outlines the primary interfaces available for developers looking to integrate Aura's download engine into their own Rust applications.

> **Live API Documentation**: You can browse the generated `rustdoc` API reference for the latest version at [ronmkr.github.io/aura/api/aura_core/](https://ronmkr.github.io/aura/api/aura_core/).

## 1. Engine & Orchestrator

The entry point for embedding Aura is the `Engine`. The engine manages the background worker threads, connection pools, and orchestrates tasks.

```rust
use aura_core::orchestrator::Engine;
use aura_core::config::Config;

let config = Config::default();
let engine = Engine::new(config).await?;
```

### Adding a Task

Tasks can be added using simple URLs, or with detailed options for multi-tenancy and priority.

```rust
use aura_core::{TaskId, TenantId};

// Simple usage
let handle = engine.add_task("https://example.com/file.iso").await?;

// Advanced multi-tenant usage
let sources = vec![("https://example.com/file.iso".to_string(), aura_core::task::TaskType::Http)];
let tenant = Some(TenantId("user_123".to_string()));

let handle = engine.add_task_with_options(
    TaskId(1),
    tenant,
    "file.iso".to_string(),
    sources,
    None,  // checksum
    100,   // priority
    false, // streaming mode
    vec![] // dependencies
).await?;
```

## 2. The `TaskHandle`

When you add a download task to the Engine, it returns a `TaskHandle`. This handle acts as your control interface for a specific running task.

```rust
pub struct TaskHandle {
    // Internal fields...
}
```

### Methods

- **`pause(&self) -> Result<()>`**
  Gracefully pauses the download. Active connections are drained, and the task state is flushed to the `.aura` control file.

- **`resume(&self) -> Result<()>`**
  Resumes a previously paused task, reloading state from the control file and aggressively finding peers or mirrors.

- **`cancel(&self) -> Result<()>`**
  Cancels the task and deletes the `.aura` control file. Optionally cleans up partially downloaded data depending on engine configuration.

- **`status(&self) -> TaskStatus`**
  Returns a snapshot of the current state, including `completed_bytes`, `total_bytes`, `download_speed`, and active connections.

## 3. Real-Time Telemetry via `Stream`

Aura utilizes `tokio_stream::Stream` to provide real-time updates for tasks. This is highly useful for building custom UIs or reacting to task lifecycle events.

### Subscribing to Events

You can obtain an event stream directly from a `TaskHandle`:

```rust
use tokio_stream::StreamExt;

let handle = engine.add_task("https://example.com/file.iso").await?;
let mut stream = handle.events();

while let Some(event) = stream.next().await {
    match event {
        TaskEvent::Progress { downloaded, total, speed } => {
            println!("Speed: {} B/s", speed);
        }
        TaskEvent::Completed => {
            println!("Download finished!");
            break;
        }
        TaskEvent::Error(err) => {
            eprintln!("Task failed: {}", err);
            break;
        }
        _ => {}
    }
}
```

### Event Types

The `TaskEvent` enum (yielded by the stream) covers the following lifecycle events:
- `Added`: The task was accepted by the Orchestrator.
- `Started`: The task has allocated space and workers have begun fetching pieces.
- `Progress { downloaded: u64, total: u64, speed: u64 }`: Periodic throughput metrics.
- `Paused`: The task was paused successfully.
- `Completed`: The download passed integrity checks and was finalized.
- `Error(String)`: A fatal error occurred that could not be self-healed.
