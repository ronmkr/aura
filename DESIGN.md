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
        value: ['', '', '', '', '', '', '', '', '', '']
---

# Aura  Design System

Aura is designed to feel fast, modern, and atmospheric. It uses a **Token-based Theming Engine** to allow users to customize their cockpit.

##  The Three Personas

Aura adapts its interface based on the user's current mission.

### 1. The Sprinter (CLI)
- **Goal**: High-speed, one-off downloads.
- **Visuals**: Minimalist. Single progress bar per file, total speed summary.
- **Workflow**: `Aura-cli <URI>`.

### 2. The Pilot (TUI)
- **Goal**: Interactive mission control.
- **Framework**: Ratatui.
- **Layout**:
    - **Header**: Global throughput (Up/Down), Active tasks count, NAT status.
    - **Main Area**: Scrollable list of tasks with interactive selection.
    - **Sidebar/Panel**: Detailed metadata for selected task (Mirrors, Peer map, Bitfield visualization).
    - **Footer**: Keybindings (a: add, p: pause, r: resume, d: delete, q: quit).
- **Theming**: CSS-like theme provider in `Aura.toml`.
- **Remote Mode**: Can "Attach" to a remote `Aura-daemon`.

### 3. The Ghost (Headless / Web)
- **Goal**: Persistent service for Docker, NAS, or Seedboxes.
- **RPC Server**: Built on Axum/Tokio (JSON-RPC 2.0 and WebSockets).
- **Compatibility**: Standardized to allow existing `aria2` frontends (like **AriaNg**) to connect with minimal adaptation.

##  Core Engine Mandates
- **Actor Integrity**: Strict decoupling via type-safe channels.
- **TDD First**: Every component must be developed using Red-Green-Refactor.
- **Zero-Copy**: Optimize for minimal memory movements via the `Buffer Pool`.
- **Atomic Completion**: Writing to `.part` files ensures no partial files are exposed.

##  Visual Identity: The Galactic Vibe

The default visual language is "Galactic". It uses high-contrast ANSI colors to ensure readability across all terminal emulators while maintaining a distinctive "precision" aesthetic.

- **Nebula Cyan**: represents the flow of data.
- **Star Yellow**: highlights the structure and critical metadata.
- **Galactic Blue**: provide a deep, stable frame for the Pilot (TUI).

##  Theming Architecture

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

### Example Palettes
1.  **Galactic (Default)**: Deep blues and cyans for a high-tech space feel.
2.  **Matrix**: Shades of neon green on black for a retro-hacker aesthetic.
3.  **Classic**: High-contrast grayscale for maximum accessibility.

##  Components

### Progress Indicators
1.  **The Pulse (Spinner)**: A fast, 100ms rhythmic animation.
2.  **The Stream (Bar)**: A `#>-` character-based bar.

### The Status Board (Table)
- **Headers**: Fixed at the top with a high-contrast background.
- **Selection**: Uses color inversion (Reversed) and the `>> ` glyph.

##  Spacing & Layout
- **Horizontal Split**: 40/20/20/20 layout for the Task Board.
- **Vertical Rhythm**: 1-cell margins between panels.

##  Interaction
- **Hotkeys**: `a` (Add), `p` (Pause), `r` (Resume), `d` (Delete), `q` (Quit).
- **Update Frequency**: 500ms UI ticks (Configurable via `tui.tick_rate_ms`).

