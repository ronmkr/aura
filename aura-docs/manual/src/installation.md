# Installation

Currently, Aura is in active development. You can build it from source using Cargo.

## Prerequisites

- **Rust**: Version 1.75 or higher.
- **OpenSSL**: Development headers (for Linux).

## Docker (Recommended for Servers & NAS)

We provide an official Dockerfile that packages the unified binary in a minimal, secure container.

### Building the Image
```bash
git clone https://github.com/ronmkr/aura.git
cd aura
docker build -t ronmkr/aura .
```

### Running the Daemon
```bash
docker run -d \
  --name aura-daemon \
  -p 6800:6800 \
  -v /path/to/your/downloads:/downloads \
  ronmkr/aura daemon
```

### Running the CLI
```bash
docker run --rm \
  -v $(pwd):/downloads \
  ronmkr/aura "https://example.com/file.iso"
```

## Cargo (From Source)

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
- `aura`: The unified binary.
- `aura daemon`: Start in background service mode.
- `aura tui`: Start the interactive dashboard.

## Verification

After installing, verify the version:
```bash
aura --version
```
You should see `Aura v0.1.0` (or higher).
