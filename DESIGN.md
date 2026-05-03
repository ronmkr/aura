---
name: Aura Design System
description: Structured design tokens and visual intent for the Aura high-performance download engine (CLI/TUI/Daemon).
tokens:
  color:
    brand:
      galactic-blue:
        value: '#0000FF' # Color::Blue in TUI
        description: Primary brand color used for surfaces and backgrounds.
      nebula-cyan:
        value: '#00FFFF' # Color::Cyan in CLI
        description: Accent color for active progress and speed indicators.
      star-yellow:
        value: '#FFFF00' # Color::Yellow in TUI
        description: Highlight color for headers and important identifiers.
    status:
      success-green:
        value: '#00FF00' # .green in CLI
        description: Indicates active, healthy, or completed states.
      error-red:
        value: '#FF0000' # .red in CLI
        description: Alerts the user to failed tasks or critical system errors.
      waiting-gray:
        value: '#808080' # Color::Gray
        description: For queued or inactive task states.
    ui:
      background:
        value: '{color.brand.galactic-blue}'
      foreground:
        value: '#FFFFFF'
      header-text:
        value: '{color.brand.star-yellow}'
      progress-bar-fill:
        value: '{color.brand.nebula-cyan}'
      progress-bar-track:
        value: '{color.brand.galactic-blue}'
  typography:
    family:
      base:
        value: 'Monospace'
        description: Fixed-width system font for consistent terminal alignment.
    size:
      base:
        value: '1ch' # 1 character cell
  spacing:
    base:
      value: '1' # 1 cell
    margin:
      header-bottom:
        value: '{spacing.base}'
  motion:
    spinner:
      duration:
        value: '100ms' # enable_steady_tick interval
      sequence:
        value: ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏']
  radii:
    none:
      value: '0' # Terminal grid is orthogonal
  elevation:
    shadow:
      none:
        value: 'none'
---

# Aura 🌌 Design System

Aura is designed to feel fast, modern, and atmospheric. As a spiritual successor to `aria2`, it transitions from a "utility tool" to a "command center" for data transport.

## 🎨 Visual Identity: The Galactic Vibe

The visual language of Aura is inspired by the vastness and precision of space. We use high-contrast ANSI colors to ensure readability across all terminal emulators while maintaining a distinctive "galactic" aesthetic.

- **Nebula Cyan**: Represents the flow of data. It is the color of the stars moving past a cockpit at warp speed.
- **Star Yellow**: Highlights the structure. Like planetary markers, it guides the user's eye to headers and critical metadata.
- **Galactic Blue**: The vacuum of space. It provides a deep, stable background for the Pilot (TUI) to operate within.

## 🎭 Personas

### The Sprinter (CLI)
The CLI design is **functional minimalism**. It prioritizes density and high-frequency updates.
- **Spinners**: Used during the "uncertainty" phase (metadata resolution).
- **Dual-tone Bars**: Cyan-on-Blue progress bars provide a high-contrast visualization of completion.
- **Metadata**: Contextual information like `bytes_per_sec` and `ETA` are displayed in-line to avoid clutter.

### The Pilot (TUI)
The TUI is an **interactive cockpit**. It uses `ratatui` to build a persistent management interface.
- **Chromeless Headers**: Headers use bold background colors rather than borders to maximize horizontal space.
- **Selection**: Uses color inversion (Reversed) and a directional glyph (`>> `) to indicate the active focus without needing complex cursor management.
- **Grids**: Standard 1-cell margins provide enough breathing room to prevent "wall of text" syndrome while maintaining data density.

## 🧱 Components

### Progress Indicators
Progress is the heartbeat of Aura.
1. **The Pulse (Spinner)**: A fast, 100ms rhythmic animation indicating background activity.
2. **The Stream (Bar)**: A `#>-` character-based bar that grows from left to right, representing the sequential filling of the local "Smart Buffer".

### The Status Board (Table)
Tables are the primary data structure for the TUI.
- **Headers**: Fixed at the top with a high-contrast background to anchor the view.
- **Rows**: Strictly 1 character high to allow managing 50+ concurrent swarms on a single screen.

## 📐 Spacing & Layout
Aura follows the **Orthogonal Grid** of the terminal. All layout decisions are based on character cells.
- **Horizontal Split**: 40/20/20/20 percentage-based layout for the Task Board to ensure filenames have primacy.
- **Vertical Rhythm**: Headers are separated from data by a single-cell bottom margin to provide visual "air".

## ⌨️ Interaction
The interface is designed for **muscle memory**.
- **Keyboard Primary**: Single-character hotkeys (`a`, `p`, `r`, `d`, `q`) allow for rapid orchestration without leaving the home row.
- **Zero-Latency Feedback**: UI ticks at 500ms intervals to ensure the dashboard feels "live" and responsive to the underlying actor engine.
