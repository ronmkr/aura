# ADR 0027: Power Management and Automated Lifecycle Actions

## Status
Accepted

## Context
Long-running downloads are often left unattended. Users expect the system to remain awake during the transfer but potentially shut down or sleep once the work is finished to save power.

## Decision
1. **Power Manager**: We will implement a `PowerManager` actor that interfaces with OS-specific power management APIs (e.g., `SetThreadExecutionState` on Windows, `IOPMVideoWakeAppearance` on macOS, or `org.freedesktop.login1` on Linux). It will hold a "power assertion" as long as there is at least one active **Download Task**.
2. **Lifecycle Controller**: A component will monitor the **Event Bus**. When the `QueueEmpty` event is received, it will check the **Configuration Manager** for any pending **Power Action**.
3. **Safety**: Before executing a destructive action like Shutdown, the `Lifecycle Controller` will publish a `FinalWarning` event, allowing a user (via TUI or RPC) to cancel the action within a short grace period.

## Alternatives Considered
- **Manual Scripts**: Relying on the `Hook Manager` to call `shutdown`. *Rejected:* Power assertions (preventing sleep) are difficult to manage via simple shell scripts and require persistent process-level integration.

## Consequences
- **Pros**: Improved energy efficiency and better unattended download experience.
- **Cons**: Requires platform-specific code for power management, which can be brittle across different OS updates.
