# RPC & Daemon Mode

Aura is designed to be a shared backbone service. While the CLI is great for one-off tasks, the **Aura Daemon** allows for persistent, remote management.

## The Aura Daemon (`aura-daemon`)

The daemon is a headless background process that:
1.  **Maintains State**: Keeps track of all active and paused tasks across sessions.
2.  **Exposes API**: Runs a JSON-RPC 2.0 server over HTTP/WebSockets.
3.  **Manages Resources**: Enforces global bandwidth limits and port mappings.

### Running the Daemon
```bash
aura-daemon --config Aura.toml
```

## JSON-RPC 2.0 API

Aura implements a standardized RPC interface compatible with many `aria2` frontends.

### Key Methods
- `aura.addUri`: Add a new download task.
- `aura.pause`: Pause a specific task.
- `aura.tellActive`: Get status of all active tasks.
- `aura.purge`: Remove completed/failed tasks.

## Security & Tokens

Connections to the RPC server are secured via **X-Aura-Token**.
- **Handshake**: Clients must include this token in the header of every request.
- **Isolation**: In multi-user mode (Milestone 7), tokens are used to isolate task lists between different tenants.

## Browser Extension Bridge

Aura includes a **Browser Bridge** (ADR 0016) that allows browser extensions (Chrome/Firefox) to funnel download requests directly into the daemon. This allows you to "Click to Download" in your browser and have Aura handle the heavy lifting in the background.
