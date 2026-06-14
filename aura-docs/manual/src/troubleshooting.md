# Troubleshooting & Common Issues

Aura is designed to be self-healing, but certain network or system conditions may require manual intervention. This guide details common error states and recovery procedures.

## Common Error Messages

### VPN & Network Safety

- **`"VPN Kill-switch triggered! Connection lost."`**
  - **Context**: `force_tunnel = true` is set, and the authorized interface (e.g., `tun0`) vanished.
  - **Action**: Aura has atomically paused all tasks to prevent data leaks. Check your VPN client. Aura will automatically resume once the interface is restored.
- **`"Captive portal detected; pausing task."`**
  - **Context**: You are on a public Wi-Fi (Hotel/Cafe) that redirected the download to a login page.
  - **Action**: Complete the login in your browser, then **Resume** the task. Aura intercepts these to prevent your download from being corrupted by HTML login pages.

### Storage & I/O

- **`"Insufficient disk space; needed X, available Y"`**
  - **Context**: Aura's pre-download verification (Decision 0060) detected that the target drive does not have enough space, including a required 5% or 512MB headroom.
  - **Action**: Free up space on the drive or change the `download_dir`. If you are using a Copy-on-Write (COW) filesystem (APFS, Btrfs), the OS-reported space may be an estimate; Aura will still enforce the reported limit to be safe.
- **`"Failed to pre-allocate file: No space left on device"`**
  - **Context**: The `StorageEngine` failed to reserve contiguous blocks via `fallocate` (likely a TOCTOU race or quota limit).
  - **Action**: Check for per-user disk quotas (`EDQUOT`).
- **`"Integrity verification failed: Checksum mismatch"`**
  - **Context**: The downloaded file does not match the provided SHA-256/MD5 hash.
  - **Action**: Aura automatically deletes the corrupted `.part` file to prevent accidental use (Decision 0061). Check the source mirror or try a different mirror if available.

### Process & System

- **`"Process Resilience: FD limit too low"`**
  - **Context**: Aura detected that the OS limit for open files is lower than required for the configured number of connections (Decision 0064).
  - **Action**:
        - **Linux/macOS**: Increase the limit using `ulimit -n 4096` before starting the daemon.
        - **Windows**: The limit is fixed at 2048 handles; reduce `max_connections_per_task` if you see connection drops.
- **`"Aura has crashed. See crash.log for details."`**
  - **Context**: An unhandled panic occurred in the engine.
  - **Action**: Aura automatically saves a backtrace to `~/.aura/crash.log` before exiting. Please include this file when reporting bugs on GitHub (see [TASKS.md](./project/TASKS.md) for known issues).

### BitTorrent Swarm

- **`"All NAT traversal methods failed for port 6881"`**
  - **Context**: UPnP and NAT-PMP/PCP requests were rejected by your router.
  - **Action**: You can still download, but you may have fewer peers (cannot receive incoming connections). Manually forward port `6881` in your router settings for "Green" status.
- **`"DHT bootstrap failed; using internal fallback"`**
  - **Context**: Primary bootstrap nodes are unreachable.
  - **Action**: Aura will automatically try a secondary list of high-uptime nodes learned from previous sessions.

---

## Performance Debugging

### TUI Lags Or High Latency

- **Cause**: Extremely large torrents with tens of thousands of files in the File Selector.
- **Solution**: Use the **Command Palette (`:`)** to filter or batch-actions rather than scrolling manually. Ensure your terminal supports GPU acceleration (e.g., Alacritty, iTerm2, Kitty).

### Actionable Error Recovery

In the **Pilot Dashboard (TUI)**, Aura often provides "Actionable Recovery" prompts. For example, if a download fails due to a disk error, the dashboard may offer a "Clear Cache" or "Retry" button that automatically handles the underlying recovery logic for you.

### High CPU Usage

- **Cause**: Extremely low `event_poll_interval_ms` (e.g., < 100ms) or high `max_peers_per_torrent`.
- **Solution**: Increase poll interval to `500ms` and cap peers to `100` in `Aura.toml`.

### Slow Download Speeds

1. **Adaptive Scaling**: Check `max_connections_per_task`. If a mirror is slow, Aura may need more connections to saturate your link.
2. **Choking**: In BitTorrent, ensure you are uploading. Aura uses a **Tit-for-Tat** algorithm; peers will "choke" you if you don't share back.
3. **Bufferbloat**: If your whole internet slows down, set a `global_upload_limit` (usually 80% of your ISP's rated upload).

---

## Advanced Logging (The Senior Way)

If you encounter an obscure bug, run Aura with structured JSON logging enabled:

```bash
# Run with Trace level for deep protocol inspection
RUST_LOG=aura=trace aura "URL"
```

To capture internal actor state transitions:

```bash
# Filter for specific components
RUST_LOG=aura_core::orchestrator=debug,aura_core::storage=info aura daemon
```

**Note**: `trace` logs are massive. Use them only for short reproduction sessions.
