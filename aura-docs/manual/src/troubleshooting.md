# Troubleshooting

This chapter covers common issues and their solutions when using Aura.

## Common Log Messages

### "VPN Kill-switch triggered! Connection lost."
- **Meaning**: Aura is configured to only use a specific network interface (e.g., `tun0`), and that interface has disappeared.
- **Solution**: Check your VPN connection. Once the interface is restored, Aura will automatically attempt to resume. If you are not using a VPN, set `vpn_kill_switch = false` in `Aura.toml`.

### "All NAT traversal methods failed for port..."
- **Meaning**: Aura attempted to open a port on your router via UPnP and NAT-PMP but failed.
- **Solution**:
    1. Ensure UPnP is enabled in your router settings.
    2. Check if your system firewall is blocking Aura.
    3. You can safely ignore this if you are not seeding or if you have manually forwarded the port.

### "Failed to pre-allocate file: No space left on device"
- **Meaning**: The destination drive is full.
- **Solution**: Free up space or change the `download_dir` in `Aura.toml`. Aura uses sparse files by default but still checks for sufficient capacity.

### "Task Error: Protocol error: Invalid redirect"
- **Meaning**: An HTTP mirror redirected Aura to an invalid URL or a captive portal.
- **Solution**: Verify the source URI. If it's a "Wait for download" page, Aura cannot currently bypass the human interaction required.

## Connectivity Issues

### BitTorrent downloads are slow
- **Cause**: No open ports for incoming peer connections.
- **Solution**: Try enabling NAT Traversal or manually forward the BitTorrent port (default: 6881) in your router.

### RPC Client cannot connect
- **Cause**: The `aura daemon` is not running or is bound to a different port.
- **Solution**: Ensure the daemon is running (`aura daemon`). Verify the port and `rpc_token` in `Aura.toml` match your client settings.

## Debugging

To get more detailed logs, run Aura with the `RUST_LOG` environment variable:
```bash
RUST_LOG=debug aura "URL"
```
Valid levels are `error`, `warn`, `info`, `debug`, and `trace`.
