# TUI Guide (pilot Dashboard)

The **Pilot Dashboard** is Aura's mission control center. It provides a highly interactive, real-time interface for managing complex downloads, visualizing performance, and performing granular task operations.

## Launching The Tui

To connect to a running `aura daemon`:

```bash
aura tui
```

By default, it attempts to connect to `127.0.0.1:6800`.

## Interface Layout (multi-view)

Aura TUI uses a **ViewRouter** architecture (Decision 0065) with three primary screens:

1. **Dashboard (Main)**: A 60/40 split-layout showing the global task list on the left and real-time details (Sparklines, Gauges) on the right.
2. **Mission Control (Task Detail)**: A deep-dive view into a single task, showing throughput history, piece availability maps, and peer distribution.
3. **File Selector**: An interactive tree-view for selecting specific files within a BitTorrent swarm or Metalink package.

### Automated Ingestion Status

Tasks added automatically via **Watch Folders** (Decision 0069) or **RSS Feed Subscriptions** (Decision 0070) appear dynamically in the main Dashboard list. Their origin/source is displayed under the task metadata section in the detail panel.

When no task is selected, the right details panel transitions into a **System Overview** dashboard displaying global status:
- **Watch Folder**: Shows whether the automated watch folder system is active or idle.
- **Last Ingested**: Displays the file name of the most recently ingested torrent or metalink file.

---

## Power-user Navigation

Aura supports lightning-fast navigation for power users:

- **Vim Motions**: Use `j/k` to move the selection, `h/l` to switch between panes or collapse/expand folders in the File Selector.
- **Command Palette (`Ctrl+P` or `:`)**: Open a fuzzy-searchable action menu. Start typing "Pause", "Remove", or "Move" to quickly execute commands without hotkeys.
- **Mouse Support**: Click to select tasks, scroll through lists, and interact with the File Selector.
- **Drag-and-Drop**: Drag a `.torrent` file or URI directly into the terminal window to ingest it (platform dependent).

---

## Key Bindings

| Key | Action |
|---|---|
| `j` / `k` | Navigate selection up/down |
| `Enter` | Open detailed **Mission Control** for selected task |
| `f` | Open **File Selector** for selected task |
| `p` | **Pause** the selected task |
| `r` | **Resume** the selected task |
| `d` / `Delete` | **Remove** the task (prompts for confirmation) |
| `/` | **Search** / Filter the task list |
| `a` | Open **Discovery Modal** (Bulk add files/folders) |
| `g` / `G` | Jump to **First / Last** task |
| `?` | Toggle **Help Overlay** |
| `Ctrl+P` / `:` | Open **Command Palette** |
| `Tab` | Switch focus between Dashboard panes |
| `Esc` | Go back one level or close modal |
| `q` | **Quit** TUI (Daemon continues in background) |

---

## Interactive File Selection

For BitTorrent swarms or Metalink files, Aura supports selective downloading:
1. Select a task and press `f`.
2. Use `j/k` to navigate the file tree.
3. Press `Space` or `Enter` to toggle a file or folder for download.
4. Press `s` to **Save and Apply** changes.
5. Press `Esc` or `h` to cancel/go back.
*Aura automatically handles shared pieces at file boundaries, ensuring data integrity while minimizing wasted disk space.*

---

## Visualizations

- **Sparklines**: Display the last 60 seconds of throughput (Download/Upload).
- **Progress Gauges**: High-resolution bars showing piece completion and verification status.
- **Piece Maps**: (In Mission Control) A 2D grid representing every piece in a torrent, color-coded by availability and download state.

---

## Theming & Os Integration

### Desktop Notifications

Aura sends native OS notifications for:
- Task completions
- Critical disk errors
- Checksum verification results

### Built-in Themes

Customize the dashboard in `Aura.toml`:
- **Galactic** (Default): Space-age blues and cyans.
- **Matrix**: Retro hacker green.
- **Nord**: Clean, arctic frost tones.

```toml
[general]
theme = "Nord"
```
