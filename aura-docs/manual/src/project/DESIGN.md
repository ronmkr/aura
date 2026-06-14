---
name: Aura Design System
description: Structured design tokens and visual intent for the Aura high-performance download engine (CLI/TUI/Daemon).
tokens:
  color:
    status:
      success:
        value: '#00FF00'
        description: Indicates active, healthy, or completed states.
      error:
        value: '#FF0000'
        description: Alerts the user to critical system errors.
      warning:
        value: '#FFFF00'
        description: Alerts for non-critical issues (e.g., retries).
      waiting:
        value: '#808080'
        description: For queued or inactive task states.
  themes:
    galactic (default):
      primary: '#0000FF' # Galactic Blue
      accent: '#00FFFF'  # Nebula Cyan
      highlight: '#FFFF00' # Star Yellow
      background: '#000000'
      foreground: '#FFFFFF'
    matrix:
      primary: '#003B00'
      accent: '#00FF41'
      highlight: '#008F11'
      background: '#000000'
      foreground: '#00FF41'
    classic:
      primary: '#FFFFFF'
      accent: '#808080'
      highlight: '#FFFFFF'
      background: '#000000'
      foreground: '#FFFFFF'
  typography:
    family:
      base:
        value: 'Monospace'
        description: Fixed-width system font for consistent terminal alignment.
    size:
      base:
        value: '1ch'
  spacing:
    base:
      value: '1'
    margin:
      header-bottom:
        value: '{spacing.base}'
  motion:
    spinner:
      duration:
        value: '100ms'
      sequence:

## value: `['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']`

# Aura Design System

Aura is designed to feel fast, modern, and atmospheric. It uses a **Token-based Theming Engine** to allow users to customize their cockpit and adapts its interface through three distinct personas.

## The Three Personas

Aura adapts its interface based on the user's current mission.

### 1. The Sprinter (cli)

- **Goal**: High-speed, one-off downloads.
- **Visuals**: Minimalist. Single progress bar per file, total speed summary.
- **Workflow**: `aura run <URI>` or `aura add <URI>`.

### 2. The Pilot (tui)

- **Goal**: Interactive mission control.
- **Framework**: Ratatui.
- **Architecture**: **Stateful Multi-View Pattern**.
- **Layout**:
  - **Dashboard**: Global throughput sparklines, active task counters, and NAT/VPN status.
  - **Task Board**: 60/40 split between the scrollable task list and the **Contextual Detail Panel**.
  - **Context Panel**: Deep metadata (Mirrors, Peer map, Bitfield visualization, Piece distribution).
  - **View Stack**: Modal overlays for the **Command Palette** (`:`) and **Fuzzy Filter** (`/`).
- **Interaction**:
  - **Vim Motions**: `j/k` for navigation, `h/l` for tab switching.
  - **Zero-Friction Ingest**: Automatic OS clipboard detection and bracketed paste support.
  - **Selective Download**: Interactive tree-view for choosing specific files from multi-file packages.

### 3. The Ghost (headless / Web)

- **Goal**: Persistent background service.
- **RPC Server**: Built on Axum/Tokio (JSON-RPC 2.0 and WebSockets).
- **Web UI**: Integrated dashboard accessible via browser, designed for remote management on NAS/Seedboxes.

## Visual Identity: The Galactic Vibe

The default visual language is "Galactic". It uses high-contrast ANSI colors to ensure readability across all terminal emulators while maintaining a distinctive "precision" aesthetic.

- **Nebula Cyan**: Represents the flow of data (Progress bars, active throughput).
- **Star Yellow**: Highlights critical metadata and active selections.
- **Galactic Blue**: Provides a stable, deep frame for the Pilot's cockpit.

## Theming Architecture

Aura supports full palette customization via `Aura.toml`. A theme is a collection of hex color mappings for the UI components.

### Customizing Your Cockpit

In your `Aura.toml`, you can define a custom theme:

```toml
[general.theme]
primary = "#0000FF"                    # Borders, Headers
accent = "#00FFFF"                     # Progress bars, Active Text
highlight = "#FFFF00"                  # Active Selection, Titles
background = "#000000"                 # TUI Background
foreground = "#FFFFFF"                 # General Text
success = "#00FF00"                    # Completed/Healthy tasks
error = "#FF0000"                      # Failed tasks
warning = "#FFFF00"                    # Retrying tasks
```

## Ui Components

### Progress Indicators

1. **The Pulse (Spinner)**: A fast, 100ms rhythmic animation (`⠋`, `⠙`, `⠹`...).
2. **The Stream (Bar)**: A `#>-` character-based bar, color-coded by status.

### The Status Board (table)

- **Sticky Headers**: Fixed at the top with a high-contrast background.
- **Reactive Selection**: Uses color inversion and the `>> ` glyph to indicate focus.

## Layout Principles

- **Grid Discipline**: 1-cell margins between all UI panels.
- **Responsive Stacking**: On small terminal windows, the sidebar collapses to prioritize the Task Board.
- **Atmospheric Depth**: Uses subtle Braille-based sparklines to visualize historical throughput without visual clutter.
