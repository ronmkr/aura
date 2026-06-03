# RPC & Daemon Mode

Aura is designed to be a shared backbone service. While the CLI is great for one-off tasks, the **Aura Daemon** (`aura daemon`) allows for persistent, remote management and multi-user isolation.

## The Aura Daemon (`aura daemon`)

The daemon is a headless background process that:
1.  **Maintains State**: Keeps track of all active and paused tasks across system restarts using `.aura` control files.
2.  **Exposes API**: Runs a JSON-RPC 2.0 server over HTTP and WebSockets (port 6800 by default).
3.  **Manages Resources**: Enforces global bandwidth limits, port mappings, and tenant isolation.

### Running the Daemon
```bash
aura daemon --rpc-port 6800 --rpc-secret "your-secret-token"
```

## JSON-RPC 2.0 API

Aura implements a standardized RPC interface. It is largely compatible with standard WebUIs and frontends.

### Core Methods
| Method | Description | Parameters |
|--------|-------------|------------|
| `aura.addUri` | Adds a new download task. | `[uris], [options]` |
| `aura.pause` | Pauses a running task. | `[gid]` |
| `aura.unpause` | Resumes a paused task. | `[gid]` |
| `aura.remove` | Removes a task and its control files. | `[gid]` |
| `aura.tellStatus`| Returns detailed status of a task. | `[gid], [keys]` |
| `aura.tellActive`| Returns status of all active tasks. | `[keys]` |
| `aura.purge` | Removes all completed/failed tasks. | None |
| `aura.getGlobalStat`| Returns global engine stats. | None |

## WebSocket Telemetry (Real-time)

For high-performance frontends (like the TUI), Aura supports **WebSocket Streaming** at `/ws`.
- **Event Bus**: Once connected, the daemon streams JSON-formatted events (e.g., `TaskAdded`, `TaskProgress`, `TaskCompleted`).
- **Efficiency**: Eliminates the need for polling `tellActive` every 500ms, significantly reducing CPU usage and network overhead for monitoring.

## Multi-Tenancy & Resource Isolation (ADR 0032)

Aura can be configured to act as a **Multi-User Hosting** engine.

### Tenant Context (`TenantContext`)
The daemon can isolate tasks using `TenantId`s. Each tenant receives:
- **Bandwidth Throttling**: Independent token-buckets for upload/download. A "heavy" user won't starve others of bandwidth.
- **Task Quotas**: Limits on the number of active tasks per user.
- **Path Sandboxing (`disk_path_root`)**: Every tenant is assigned a specific root directory. Aura strictly enforces that no download or mapping rule can escape this sandbox.

## Security & Tokens

Connections are secured via the **X-Aura-Token** header.
- **Auth Handshake**: Every RPC call must provide the secret token configured in `Aura.toml`.
- **Tenant Mapping**: In multi-tenant environments, the token is used to automatically map the request to the correct `TenantContext`, ensuring a user can only see or manage their own tasks.

## Browser Extension Bridge

The **Browser Bridge** (ADR 0016) allows Chrome and Firefox extensions to communicate with the daemon via a local RPC loop. 
- **Protocol Takeover**: Aura can be set as the default handler for `magnet:`, `.torrent`, and `.metalink` URIs.
- **One-Click Download**: Clicking a download link in your browser instantly spawns a background task in the daemon, appearing in your TUI dashboard automatically.
