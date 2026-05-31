# Built-in Web UI

In addition to the CLI and TUI, the Aura Daemon includes a built-in **Web Dashboard** for managing downloads from any modern browser.

## Accessing the Web UI

By default, the Web UI is served by the daemon on the same port as the RPC server.

1.  Start the daemon: `aura daemon`
2.  Open your browser to: `http://127.0.0.1:6800/ui/`

## Features

- **Real-time Monitoring**: Powered by WebSockets for instant progress updates.
- **Drag-and-Drop**: Upload `.torrent` or `.metalink` files directly through the browser.
- **Global Settings**: Update bandwidth limits and network configurations on the fly.
- **Responsive Design**: Works on mobile and desktop browsers.

## Security

Access to the Web UI is protected by the same **X-Aura-Token** as the JSON-RPC API. Upon first visit, you will be prompted to enter your secret token.

For multi-tenant environments, the Web UI automatically filters tasks based on the user's token, providing a personalized dashboard for every tenant.
