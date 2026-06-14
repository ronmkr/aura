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

## Running as a System Service (Decision 0071)

For permanent installations (Servers, NAS, Seedboxes), we recommend running the Aura daemon as a system service. This ensures the engine starts automatically on boot and recovers from unexpected process terminations.

### Installation
```bash
sudo aura service install
```
*Note: On Linux/macOS, this usually requires root privileges to register the service with systemd or launchd.*

### Control Commands
- **Start**: `aura service start`
- **Stop**: `aura service stop`
- **Status**: `aura service status`
- **Uninstall**: `sudo aura service uninstall`

### Platform Support
- **Linux**: Integrates with `systemd` (creates `aura.service`).
- **macOS**: Integrates with `launchd` (creates `com.aura.daemon.plist`).
- **Windows**: Integrates with the **Service Control Manager** (via `install-service.ps1`).

## Feature Flags

When building from source via Cargo, you can toggle optional capabilities using Cargo feature flags:

- **`s3`**: Enables support for S3-compatible cloud storage targets.
- **`gdrive`**: Enables support for Google Drive and OneDrive storage targets.
- **`nntp`**: Enables experimental Usenet (NNTP) protocol worker stubbing.

To build Aura with all features enabled:
```bash
cargo build --release --features "s3 gdrive"
```

## Configuration & Data Directory

Aura stores its persistent state, history, and logs in a hidden directory:
- **Linux/macOS**: `~/.aura/`
- **Windows**: `%AppData%\aura\`

### Key Files:
- `tasks/`: Directory containing active task state files (`.json`).
- `history.jsonl`: Append-only download history log.
- `crash.log`: Emergency backtrace reports from the panic hook.
- `rpc_secret`: (Auto-generated) The secret token for the RPC API.

## Verification

After installing, verify the version:
```bash
aura --version
```
You should see `Aura v0.1.0` (or higher).
