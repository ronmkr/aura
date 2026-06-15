# Decision 0074: Desktop OS Notifications

## Status

Implemented (2026-06-11, commit `9e86833`)

## Context

Aura is designed to run headlessly as a background daemon, as well as interactively through a TUI or CLI interface. Users need to be informed when a long-running download task finishes or encounters a terminal error without constantly polling the status. We need a cross-platform solution to deliver system-native notifications.

## Decision

1. **Native OS Notifications**: We will use the `notify-rust` crate to send native notifications via DBus on Linux, Notification Center on macOS, and Toast notifications on Windows.
2. **Centralized Notification Service**: We will implement a `NotificationService` in the `orchestrator/notifications.rs` module that handles notification dispatching.
3. **Core Event Triggers**: The `Orchestrator` will trigger desktop alerts on:
   - **Task Complete**: "Download Completed" with filename and size.
   - **Task Error**: "Download Failed" with the reason/error message.
4. **Configurable Behavior**: Users can toggle notifications globally and configure sound/urgency via the `[notifications]` section in `Aura.toml`.

## Alternatives Considered

- **Custom DBus / AppleScript IPC**: Writing custom handlers for each OS interface. *Rejected:* Introducing custom platform logic increases technical debt compared to a well-maintained library like `notify-rust`.
- **TUI-only Visual Alerts**: Only flashing the TUI screen or displaying status. *Rejected:* Does not help when the terminal is minimized or when running in daemon-only mode.

## Consequences

- **Pros**: Zero-friction background notifications for users, native OS integration, simple configuration.
- **Cons**: Adds `notify-rust` as a dependency, which depends on platform-specific C libraries on Linux (libdbus) but these are standard on modern desktop systems.

## Implementation

- **Notification Service**: Code resides in `aura-core/src/orchestrator/notifications.rs`.
- **Configuration Module**: Defined in `aura-core/src/config/notifications.rs`.
