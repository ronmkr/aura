# Installation

Currently, Aura is in active development. You can build it from source using Cargo.

## Prerequisites

- **Rust**: Version 1.75 or higher.
- **OpenSSL**: Development headers (for Linux).

## Building from Source

1. Clone the repository:
   ```bash
   git clone https://github.com/ronmkr/aura.git
   cd aura
   ```

2. Build the workspace:
   ```bash
   cargo build --release
   ```

The binaries will be available in `target/release/`:
- `aura-cli`: The command-line interface.
- `aura-daemon`: The headless background service.
- `aura-tui`: The interactive terminal dashboard.
