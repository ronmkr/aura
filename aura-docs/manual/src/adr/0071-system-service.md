# Decision 0071: System Service Integration

## Status

Implemented (2026-06-14 — PR #303)

## Context

Aura's daemon (`daemon_mode = true`) currently requires manual execution or Docker containerization (Decision-0051) to run. For native (non-containerized) deployments on bare metal, virtual machines, or local desktop systems, there is no built-in way to auto-start the daemon on system boot. A daemon that requires user login or manual console execution is not production-ready for servers. System managers like systemd (Linux), launchd (macOS), and Windows Service Control Manager (Windows) are standard system utilities that should be supported.

## Decision

1. Add service packaging files to the repository under `packaging/`:
   - `packaging/aura.service`: Standard systemd service file for Linux system initialization.
   - `packaging/com.aura.daemon.plist`: launchd configuration file for macOS user agents.
   - `packaging/install-service.ps1`: PowerShell helper script to install Aura as a Windows Service.
2. Extend the `aura` command-line interface with `aura daemon` subcommands:
   - `aura daemon install`: Copies service configuration files to system directories and registers the daemon.
   - `aura daemon uninstall`: Stops and unregisters the service, removing configuration files.
   - `aura daemon status`: Interacts with system managers (`systemctl`, `launchctl`, `sc.exe`) to display service health.
3. The installer subcommand will detect the host platform and direct configuration to standard system folders (e.g., `/etc/systemd/system/` on Linux, `~/Library/LaunchAgents/` on macOS).
4. System service units will launch the daemon with proper arguments (e.g., `aura daemon --start` or using environment variables) and set up restart policies (e.g., `Restart=on-failure` with 5-second restart delay).

## Edge Cases

1. **Privilege Escalation**: Installing systemd or Windows services requires root/Administrator privileges. The CLI must check for privileges on launch and either print a clear error ("Must run as root/Administrator") or trigger platform-specific elevation prompts.
2. **Log Redirection**: When running as a system service, stdout/stderr are redirected to system journals (journald, Event Viewer, syslog). The daemon must ensure that logging outputs to stderr in unstructured plain-text format (or JSON) when a service environment variable is set.
3. **Paths and Configurations**: System services run under distinct user contexts (e.g., `nobody`, `system`). Ensure the configuration paths (`~/.aura/Aura.toml`) resolve correctly relative to the user starting the service.

## Alternatives Considered

- **Docker Only**: Only support daemon persistence via Docker. *Rejected:* Prevents direct integration on lightweight hosts, NAS systems lacking Docker (e.g., low-end ARM boards), and native Windows/macOS desktop users.
- **Manual init.d Scripts**: Support legacy sysvinit. *Rejected:* Modern Linux standard is systemd; sysvinit adds unnecessary shell script maintenance.

## Consequences

- **Pros**: Enables robust native daemon persistence across reboots; simple installation experience using CLI commands; improves native OS integration.
- **Cons**: Requires executing platform-specific CLI command wrappers (`systemctl`, `launchctl`, `sc.exe`) inside the Rust codebase.
