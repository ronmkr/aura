# Troubleshooting & Common Issues

Aura is designed to be self-healing, but certain network or system conditions may require manual intervention. This guide details common error states and recovery procedures.

## Common Error Messages

### 🛡️ VPN & Network Safety
- **`"VPN Kill-switch triggered! Connection lost."`**
    - **Context**: `force_tunnel = true` is set, and the authorized interface (e.g., `tun0`) vanished.
    - **Action**: Aura has atomically paused all tasks to prevent data leaks. Check your VPN client. Aura will automatically resume once the interface is restored.
- **`"Captive portal detected; pausing task."`**
    - **Context**: You are on a public Wi-Fi (Hotel/Cafe) that redirected the download to a login page.
    - **Action**: Complete the login in your browser, then **Resume** the task. Aura intercepts these to prevent your download from being corrupted by HTML login pages.

### 💾 Storage & I/O
- **`"Failed to pre-allocate file: No space left on device"`**
    - **Context**: The `StorageEngine` failed to reserve contiguous blocks via `fallocate`.
    - **Action**: Free up space or move the `download_dir`. Aura verifies space *before* starting to prevent mid-download failures.
- **`"Integrity verification failed: Checksum mismatch"`**
    - **Context**: The downloaded file does not match the provided SHA-256/MD5 hash.
    - **Action**: Aura preserves the corrupted file. Check the source mirror. If using BitTorrent, the **Integrity Scrubber** will automatically re-download corrupt pieces.

### 🕸️ BitTorrent Swarm
- **`"All NAT traversal methods failed for port 6881"`**
    - **Context**: UPnP and NAT-PMP/PCP requests were rejected by your router.
    - **Action**: You can still download, but you may have fewer peers (cannot receive incoming connections). Manually forward port `6881` in your router settings for "Green" status.
- **`"DHT bootstrap failed; using internal fallback"`**
    - **Context**: Primary bootstrap nodes are unreachable.
    - **Action**: Aura will automatically try a secondary list of high-uptime nodes learned from previous sessions.

---

## Performance Debugging

### High CPU Usage
- **Cause**: Extremely low `event_poll_interval_ms` (e.g., < 100ms) or high `max_peers_per_torrent`.
- **Solution**: Increase poll interval to `500ms` and cap peers to `100` in `Aura.toml`.

### Slow Download Speeds
1.  **Adaptive Scaling**: Check `max_connections_per_task`. If a mirror is slow, Aura may need more connections to saturate your link.
2.  **Choking**: In BitTorrent, ensure you are uploading. Aura uses a **Tit-for-Tat** algorithm; peers will "choke" you if you don't share back.
3.  **Bufferbloat**: If your whole internet slows down, set a `global_upload_limit` (usually 80% of your ISP's rated upload).

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
