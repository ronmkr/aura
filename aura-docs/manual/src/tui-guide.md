# TUI Guide (The Pilot Dashboard)

The **Galactic Pilot Dashboard** is Aura's interactive terminal interface. It provides a real-time view of all tasks managed by the Aura Daemon.

## Launching the TUI

To connect to a running `aura-daemon`:
```bash
aura-tui
```
By default, it attempts to connect to `127.0.0.1:6800`.

## Interface Layout

- **Header**: Shows the engine version and current status.
- **Task Table**: A detailed list of all downloads (Active, Paused, Error, Complete).
    - **Name**: The filename being downloaded.
    - **Status**: The current lifecycle phase.
    - **Progress**: Percentage completion.
    - **Size**: Total file size.
    - **GID**: The internal Task ID.
- **Footer**: Interactive command bar and error notifications.

## Key Bindings

| Key | Action |
|---|---|
| `j` / `Down` | Navigate down the task list |
| `k` / `Up` | Navigate up the task list |
| `p` | **Pause** the selected task |
| `r` | **Resume** the selected task |
| `q` | **Quit** the TUI (Daemon remains running) |

## Theming

The TUI supports fully customizable themes via `Aura.toml`.

```toml
[theme]
primary = "#00FF00" # Emerald
background = "#0A0A0A" # Deep Space
accent = "#FF00FF" # Pulsar Pink
```

See the [Configuration](./configuration.md) chapter for all themeable keys.
